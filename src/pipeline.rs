use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use crate::cache::{self, CacheEvent, CacheIndex, CacheOutcome};
use crate::model::{DefKind, FileDefs};
use crate::parser::MatchMode;
use crate::registry::ParserRegistry;

const PARSE_FAILURE_MSG: &str = "tree-sitter parse failure";

pub struct FileError {
    pub path: PathBuf,
    pub message: String,
}

pub struct SearchResult {
    pub definitions: Vec<FileDefs>,
    pub read_errors: Vec<FileError>,
    pub parse_failures: Vec<FileError>,
}

#[derive(Clone, Default)]
pub struct SearchOptions {
    pub hidden: bool,
    pub no_ignore: bool,
    pub max_depth: Option<usize>,
    pub max_scope_depth: Option<usize>,
    pub project_root: Option<PathBuf>,
}

pub fn search(
    modes: &[MatchMode],
    kinds: &[DefKind],
    paths: &[&Path],
    globs: &[String],
    languages: &[String],
    options: &SearchOptions,
    registry: &ParserRegistry,
) -> anyhow::Result<SearchResult> {
    // Convert relative paths to absolute. When project_root is specified,
    // relative paths resolve against project_root (not cwd). Otherwise, cwd.
    // Uses join (not canonicalize) to avoid UNC \\?\ paths on Windows.
    let path_base = options
        .project_root
        .as_ref()
        .cloned()
        .or_else(|| std::env::current_dir().ok())
        .unwrap_or_else(|| PathBuf::from("."));
    let abs_paths: Vec<PathBuf> = paths
        .iter()
        .map(|p| {
            if p.is_absolute() {
                p.to_path_buf()
            } else {
                path_base.join(p)
            }
        })
        .collect();

    if abs_paths.is_empty() {
        anyhow::bail!("no search paths provided");
    }

    // Single WalkBuilder for all paths (ripgrep pattern: first path via new(), rest via add()).
    let mut builder = ignore::WalkBuilder::new(&abs_paths[0]);
    for p in abs_paths.iter().skip(1) {
        builder.add(p);
    }
    builder
        .hidden(!options.hidden)
        .git_ignore(!options.no_ignore)
        .git_global(!options.no_ignore)
        .git_exclude(!options.no_ignore)
        .ignore(!options.no_ignore);
    if let Some(depth) = options.max_depth {
        builder.max_depth(Some(depth));
    }

    // Use first path's absolute version as WalkBuilder root for OverrideBuilder.
    let root = abs_paths[0].clone();

    // Extension filter: always applied via types(), AND'd with overrides and ignore rules
    let lang_strs: Vec<&str> = languages.iter().map(|s| s.as_str()).collect();
    let mut extensions = registry.supported_extensions_for_kinds(kinds);
    if !lang_strs.is_empty() {
        let lang_exts = registry.supported_extensions_for_languages(&lang_strs);
        extensions.retain(|e| lang_exts.contains(e));
    }
    if !extensions.is_empty() {
        let mut types = ignore::types::TypesBuilder::new();
        for ext in &extensions {
            types
                .add("supported", &format!("*.{}", ext))
                .expect("Invalid extension glob");
        }
        types.select("supported");
        builder.types(types.build().expect("Failed to build types"));
    }

    // User glob: applied via overrides(), AND'd with types and ignore rules.
    // Later globs override earlier globs; '!' prefix negates.
    if !globs.is_empty() {
        let mut overrides = ignore::overrides::OverrideBuilder::new(&root);
        for glob in globs {
            overrides
                .add(glob)
                .map_err(|e| anyhow::anyhow!("invalid glob pattern '{}': {}", glob, e))?;
        }
        builder.overrides(overrides.build().expect("Failed to build overrides"));
    }

    // Phase 1: Cache preparation — use project_root from SearchOptions.
    // project_root determines: cache location, default search scope, relative path base.
    // When None, cache is disabled (search still works, just slower).
    let project_root = options
        .project_root
        .clone()
        .or_else(|| cache::resolve_project_root(None));
    let cache_path = project_root
        .as_ref()
        .map(|pr| pr.join(".peek-cache").join("cache.bin"));
    let cache_index: Option<CacheIndex> = cache_path.as_ref().and_then(|cp| CacheIndex::load(cp));
    if cache_index.is_none() {
        if let Some(cp) = cache_path.as_ref().filter(|p| p.exists()) {
            let _ = std::fs::remove_file(cp);
        }
    }

    // Concurrent result collectors
    let results: Mutex<Vec<FileDefs>> = Mutex::new(Vec::new());
    let cache_events: Mutex<Vec<(u64, CacheEvent)>> = Mutex::new(Vec::new());
    let read_errors: Mutex<Vec<FileError>> = Mutex::new(Vec::new());
    let parse_failures: Mutex<Vec<FileError>> = Mutex::new(Vec::new());

    // Phase 2: Fused parallel walk + processing (ripgrep-style build_parallel)
    builder.build_parallel().run(|| {
        // Factory: called once per thread — init thread-local parser cache
        let mut parser_cache: HashMap<&'static str, tree_sitter::Parser> = HashMap::new();
        // Non-Copy types: capture as references (factory is FnMut, called once per thread)
        let project_root = &project_root;
        let cache_index = &cache_index;
        let modes = &modes;
        let lang_hints = &lang_strs;
        let results = &results;
        let cache_events = &cache_events;
        let read_errors = &read_errors;
        let parse_failures = &parse_failures;

        Box::new(
            move |entry: Result<ignore::DirEntry, ignore::Error>| -> ignore::WalkState {
                // Handle traversal errors (permission denied, etc.)
                let entry = match entry {
                    Ok(e) => e,
                    Err(err) => {
                        let path = match &err {
                            ignore::Error::WithPath { path, .. } => path.clone(),
                            _ => PathBuf::from("?"),
                        };
                        read_errors.lock().unwrap().push(FileError {
                            path,
                            message: err.to_string(),
                        });
                        return ignore::WalkState::Continue;
                    }
                };

                // Check if regular file (uses cached file_type from readdir, no extra stat)
                if !entry.file_type().is_some_and(|ft| ft.is_file()) {
                    return ignore::WalkState::Continue;
                }

                let path = entry.path();

                // Extension filter
                let parser = match registry.get_by_ext(path, lang_hints) {
                    Some(p) => p,
                    None => return ignore::WalkState::Continue,
                };

                // Get or create thread-local tree-sitter parser
                let ts_parser = parser_cache
                    .entry(parser.language())
                    .or_insert_with(|| parser.init_parser());

                // Own the path once (avoid repeated to_path_buf allocations)
                let path_buf = path.to_path_buf();

                // Compute path_hash for cache lookup/update.
                // Files outside project_root get rel=None, ph=None → uncached path.
                let rel = project_root
                    .as_ref()
                    .and_then(|pr| path.strip_prefix(pr).ok());
                let ph = rel.map(cache::path_hash);

                // --- Cached path ---
                if let (Some(ci), Some(ph_val), Some(_rel_path)) = (cache_index.as_ref(), ph, rel) {
                    let file_meta = match std::fs::metadata(path) {
                        Ok(meta) => meta,
                        Err(e) => {
                            read_errors.lock().unwrap().push(FileError {
                                path: path_buf.clone(),
                                message: e.to_string(),
                            });
                            return ignore::WalkState::Continue;
                        }
                    };
                    let file_mtime = cache::mtime_millis(&file_meta);
                    let file_size = file_meta.len();

                    if let Some(mtime) = file_mtime {
                        match ci.lookup(ph_val, mtime, file_size) {
                            CacheOutcome::Hit(cache_entry) => {
                                if let Some(defs) =
                                    cache::decode_data(ci.mapped_bytes(), &cache_entry)
                                {
                                    let mut file_defs = FileDefs {
                                        file: path_buf.clone(),
                                        defs,
                                    };
                                    filter_file_defs(
                                        &mut file_defs,
                                        modes,
                                        kinds,
                                        options.max_scope_depth,
                                    );
                                    cache_events.lock().unwrap().push((
                                        cache_entry.path_hash(),
                                        CacheEvent::Hit(cache_entry),
                                    ));
                                    results.lock().unwrap().push(file_defs);
                                    return ignore::WalkState::Continue;
                                }
                            }
                            CacheOutcome::Stale(_) | CacheOutcome::NotFound => {}
                        }
                    }

                    // Cache miss/stale/decode-failure: stat first, then read and parse
                    let file_meta = std::fs::metadata(path).ok();
                    let source = match std::fs::read_to_string(path) {
                        Ok(s) => s,
                        Err(e) => {
                            read_errors.lock().unwrap().push(FileError {
                                path: path_buf.clone(),
                                message: e.to_string(),
                            });
                            return ignore::WalkState::Continue;
                        }
                    };
                    match parser.extract_with(&MatchMode::All, DefKind::all(), &source, ts_parser) {
                        Ok(mut defs) => {
                            cache::truncate_defs(&mut defs);
                            let mut file_defs = FileDefs {
                                file: path_buf.clone(),
                                defs: defs.clone(),
                            };
                            filter_file_defs(&mut file_defs, modes, kinds, options.max_scope_depth);
                            let event = file_meta
                                .as_ref()
                                .and_then(|m| cache::mtime_millis(m).map(|t| (t, m.len())))
                                .map(|(mtime, size)| CacheEvent::Miss {
                                    path_hash: ph_val,
                                    mtime,
                                    size,
                                    defs,
                                });
                            if let Some(event) = event {
                                cache_events.lock().unwrap().push((ph_val, event));
                            }
                            results.lock().unwrap().push(file_defs);
                        }
                        Err(()) => {
                            parse_failures.lock().unwrap().push(FileError {
                                path: path_buf.clone(),
                                message: PARSE_FAILURE_MSG.to_string(),
                            });
                        }
                    }
                    return ignore::WalkState::Continue;
                }

                // --- Uncached path ---
                let file_meta = match std::fs::metadata(path) {
                    Ok(meta) => meta,
                    Err(e) => {
                        read_errors.lock().unwrap().push(FileError {
                            path: path_buf.clone(),
                            message: e.to_string(),
                        });
                        return ignore::WalkState::Continue;
                    }
                };
                let file_mtime = cache::mtime_millis(&file_meta);
                let file_size = file_meta.len();
                let source = match std::fs::read_to_string(path) {
                    Ok(s) => s,
                    Err(e) => {
                        read_errors.lock().unwrap().push(FileError {
                            path: path_buf.clone(),
                            message: e.to_string(),
                        });
                        return ignore::WalkState::Continue;
                    }
                };

                match parser.extract_with(&MatchMode::All, DefKind::all(), &source, ts_parser) {
                    Ok(mut defs) => {
                        cache::truncate_defs(&mut defs);
                        let mut file_defs = FileDefs {
                            file: path_buf.clone(),
                            defs: defs.clone(),
                        };
                        filter_file_defs(&mut file_defs, modes, kinds, options.max_scope_depth);
                        if let (Some(ph_val), Some(mtime)) = (ph, file_mtime) {
                            cache_events.lock().unwrap().push((
                                ph_val,
                                CacheEvent::Miss {
                                    path_hash: ph_val,
                                    mtime,
                                    size: file_size,
                                    defs,
                                },
                            ));
                        }
                        results.lock().unwrap().push(file_defs);
                    }
                    Err(()) => {
                        parse_failures.lock().unwrap().push(FileError {
                            path: path_buf.clone(),
                            message: PARSE_FAILURE_MSG.to_string(),
                        });
                    }
                }
                ignore::WalkState::Continue
            },
        )
    });

    let results = results.into_inner().unwrap();
    let cache_events = cache_events.into_inner().unwrap();
    let read_errors = read_errors.into_inner().unwrap();
    let parse_failures = parse_failures.into_inner().unwrap();

    // Phase 3: Serial cache update — build buffer, release mmap, write atomically.
    // Two-step write: build buffer while mmap is alive (reads old data),
    // then drop mmap before rename (Windows locks mmap'd files).
    // Only write when project_root exists (no cache outside a project).
    let has_non_hit = cache_events
        .iter()
        .any(|(_, e)| !matches!(e, CacheEvent::Hit(_)));
    if has_non_hit {
        if let Some(ref cache_path) = cache_path {
            let updates: HashMap<u64, CacheEvent> = cache_events.into_iter().collect();
            match cache::build_cache_buffer(cache_index.as_ref(), &updates) {
                Ok(buf) => {
                    drop(cache_index);
                    if cache::write_cache_atomic(cache_path, &buf).is_ok() {
                        let old_v3_dir = cache_path
                            .parent()
                            .map(|p| p.join("files"))
                            .filter(|p| p.exists());
                        if let Some(v3_dir) = old_v3_dir {
                            if let Err(e) = std::fs::remove_dir_all(&v3_dir) {
                                eprintln!(
                                    "peek: warning: failed to clean up legacy cache directory '{}': {}",
                                    v3_dir.display(),
                                    e
                                );
                            }
                        }
                    } else {
                        eprintln!(
                            "peek: warning: failed to write cache '{}'",
                            cache_path.display()
                        );
                    }
                }
                Err(_) => {
                    eprintln!(
                        "peek: warning: failed to build cache '{}'",
                        cache_path.display()
                    );
                }
            }
        }
    }

    // Remove empty FileDefs (files where all definitions were filtered out),
    // sort by file path for deterministic output, and dedup accessor definitions.
    let mut definitions: Vec<FileDefs> = results
        .into_iter()
        .filter(|fd| !fd.defs.is_empty())
        .collect();
    definitions.sort_by(|a, b| a.file.cmp(&b.file));
    crate::output::dedup_accessors(&mut definitions);

    Ok(SearchResult {
        definitions,
        read_errors,
        parse_failures,
    })
}

/// Filter definitions within a FileDefs by kinds and match mode.
/// Retains matching definitions in place; does not remove the FileDefs even if empty.
fn filter_file_defs(
    file_defs: &mut FileDefs,
    modes: &[MatchMode],
    kinds: &[DefKind],
    max_scope_depth: Option<usize>,
) {
    file_defs.defs.retain(|d| {
        kinds.contains(&d.kind)
            && modes.iter().any(|m| m.matches_ident(&d.scope))
            && max_scope_depth.is_none_or(|max| scope_depth(&d.scope) <= max)
    });
}

fn scope_depth(scope: &str) -> usize {
    let mut depth = 1;
    let mut chars = scope.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '.' | '\\' => depth += 1,
            ':' if chars.peek() == Some(&':') => {
                chars.next();
                depth += 1;
            }
            _ => {}
        }
    }
    depth
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{DefContent, DefKind};
    use crate::parser::MatchMode;
    use crate::parser::python::PythonParser;
    use crate::pattern::{CaseSensitivity, ParsedPattern};
    use crate::registry::ParserRegistry;
    use std::path::Path;

    fn build_registry() -> ParserRegistry {
        let mut reg = ParserRegistry::new();
        reg.register(Box::new(PythonParser));
        reg
    }

    fn parse_mode(name: &str) -> Vec<MatchMode> {
        vec![
            ParsedPattern::parse(name, CaseSensitivity::Sensitive, false)
                .unwrap()
                .mode()
                .clone(),
        ]
    }

    #[test]
    fn search_finds_python_function_in_fixtures() {
        let reg = build_registry();
        let modes = parse_mode("top_level_func");
        let results = search(
            &modes,
            &[DefKind::Function],
            &[Path::new("tests/fixtures/python")],
            &[],
            &[],
            &SearchOptions::default(),
            &reg,
        )
        .unwrap();
        assert!(!results.definitions.is_empty());
        let all_defs: Vec<&DefContent> =
            results.definitions.iter().flat_map(|fd| &fd.defs).collect();
        assert_eq!(all_defs[0].kind, DefKind::Function);
        assert_eq!(all_defs[0].scope, "top_level_func");
    }

    #[test]
    fn search_finds_python_class_in_fixtures() {
        let reg = build_registry();
        let modes = parse_mode("MyClass");
        let results = search(
            &modes,
            &[DefKind::Class],
            &[Path::new("tests/fixtures/python")],
            &[],
            &[],
            &SearchOptions::default(),
            &reg,
        )
        .unwrap();
        assert!(!results.definitions.is_empty());
        let all_defs: Vec<&DefContent> =
            results.definitions.iter().flat_map(|fd| &fd.defs).collect();
        assert_eq!(all_defs[0].scope, "MyClass");
    }

    #[test]
    fn search_returns_empty_for_unknown_name() {
        let reg = build_registry();
        let modes = parse_mode("does_not_exist_anywhere");
        let results = search(
            &modes,
            DefKind::all(),
            &[Path::new("tests/fixtures")],
            &[],
            &[],
            &SearchOptions::default(),
            &reg,
        )
        .unwrap();
        let total: usize = results.definitions.iter().map(|fd| fd.defs.len()).sum();
        assert_eq!(total, 0);
    }

    #[test]
    fn results_grouped_by_file() {
        let reg = build_registry();
        let modes = parse_mode("MyClass");
        let results = search(
            &modes,
            DefKind::all(),
            &[Path::new("tests/fixtures")],
            &[],
            &[],
            &SearchOptions::default(),
            &reg,
        )
        .unwrap();
        // Each FileDefs has a unique file path
        for fd in &results.definitions {
            if !fd.defs.is_empty() {
                // Lines within a file should be in order (tree-sitter outputs in order)
                for window in fd.defs.windows(2) {
                    assert!(
                        window[0].lines[0] <= window[1].lines[0],
                        "Defs not in line order within file {:?}: {:?} before {:?}",
                        fd.file,
                        window[0].lines,
                        window[1].lines
                    );
                }
            }
        }
    }

    // --- Multi-path search ---

    #[test]
    fn search_multi_path_works() {
        let reg = build_registry();
        let modes = parse_mode("MyClass");
        let results = search(
            &modes,
            DefKind::all(),
            &[Path::new("tests/fixtures/python")],
            &[],
            &[],
            &SearchOptions::default(),
            &reg,
        )
        .unwrap();
        assert!(!results.definitions.is_empty());
        // Multi-path with overlapping directories: parallel walk does not dedup (ripgrep-consistent)
        let results2 = search(
            &modes,
            DefKind::all(),
            &[
                Path::new("tests/fixtures/python"),
                Path::new("tests/fixtures/python"),
            ],
            &[],
            &[],
            &SearchOptions::default(),
            &reg,
        )
        .unwrap();
        assert!(results2.definitions.len() >= results.definitions.len());
    }

    // --- Cache integration ---

    #[test]
    fn cache_files_written_to_disk_with_relative_path() {
        // Bug #1 regression test: cache must work when search path is relative.
        // Cache write itself is tested in cache.rs with tempdir isolation;
        // this test verifies search succeeds with relative paths (the original bug).
        let reg = build_registry();
        let modes = parse_mode("top_level_func");

        let results = search(
            &modes,
            &[DefKind::Function],
            &[Path::new("tests/fixtures/python")],
            &[],
            &[],
            &SearchOptions::default(),
            &reg,
        )
        .unwrap();
        assert!(!results.definitions.is_empty());
    }

    // --- Old cache migration ---

    // NOTE: v3 cache migration is tested in cache.rs unit tests with tempdir isolation.
    // Pipeline-level migration depends on write_cache success and cannot be reliably
    // tested here due to parallel test interference on the shared .peek-cache directory.

    // --- Cache hit with All mode (survey) ---

    #[test]
    fn cache_hit_with_all_mode_produces_correct_results() {
        let reg = build_registry();

        // First search with MatchMode::All (survey mode) populates cache
        let modes_all = vec![MatchMode::All];
        let results_all = search(
            &modes_all,
            DefKind::all(),
            &[Path::new("tests/fixtures/python")],
            &[],
            &[],
            &SearchOptions::default(),
            &reg,
        )
        .unwrap();

        // Second search should produce same results regardless of cache state
        let results_hit = search(
            &modes_all,
            DefKind::all(),
            &[Path::new("tests/fixtures/python")],
            &[],
            &[],
            &SearchOptions::default(),
            &reg,
        )
        .unwrap();

        let all_count: usize = results_all.definitions.iter().map(|fd| fd.defs.len()).sum();
        let hit_count: usize = results_hit.definitions.iter().map(|fd| fd.defs.len()).sum();
        assert_eq!(
            all_count, hit_count,
            "cache hit should produce same number of definitions as cache miss"
        );
    }

    // NOTE: corrupt cache deletion is tested in cache.rs with tempdir isolation.
    // Pipeline-level testing of this behavior requires CWD manipulation which
    // breaks parallel test execution — the cache-level test is sufficient.
}
