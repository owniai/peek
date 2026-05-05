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

#[derive(Copy, Clone, Default)]
pub struct SearchOptions {
    pub hidden: bool,
    pub no_ignore: bool,
    pub max_depth: Option<usize>,
    pub max_scope_depth: Option<usize>,
}

pub fn search(
    modes: &[MatchMode],
    kinds: &[DefKind],
    paths: &[&Path],
    globs: &[String],
    options: &SearchOptions,
    registry: &ParserRegistry,
) -> anyhow::Result<SearchResult> {
    // Convert relative paths to absolute using current_dir().join() (not canonicalize,
    // which produces UNC \\?\ paths on Windows incompatible with ignore::WalkBuilder).
    // This ensures WalkBuilder produces absolute paths that align with CacheManager's
    // project_root, making strip_prefix work correctly.
    let abs_paths: Vec<PathBuf> = paths
        .iter()
        .map(|p| {
            if p.is_absolute() {
                p.to_path_buf()
            } else {
                std::env::current_dir()
                    .unwrap_or_else(|_| p.to_path_buf())
                    .join(p)
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
    let extensions = registry.supported_extensions_for_kinds(kinds);
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

    // Phase 1: Cache preparation — find project root from CWD (not search path),
    // load existing cache.bin if present. No project root → no caching.
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let project_root = cache::find_project_root(&cwd);
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
                let parser = match registry.get_by_ext(path) {
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

    // Remove empty FileDefs (files where all definitions were filtered out)
    let definitions: Vec<FileDefs> = results
        .into_iter()
        .filter(|fd| !fd.defs.is_empty())
        .collect();

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
    use crate::model::{DefContent, DefKind, FileDefs};
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

    fn make_fd(defs: Vec<DefContent>) -> FileDefs {
        FileDefs {
            file: PathBuf::from("f.rs"),
            defs,
        }
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
            &SearchOptions::default(),
            &reg,
        )
        .unwrap();
        assert!(results2.definitions.len() >= results.definitions.len());
    }

    // --- filter_definitions ---

    #[test]
    fn filter_by_kind_only() {
        let mut fd = make_fd(vec![
            DefContent {
                kind: DefKind::Function,
                lines: [1, 1],
                signature: "fn a".into(),
                scope: "a".into(),
            },
            DefContent {
                kind: DefKind::Class,
                lines: [2, 2],
                signature: "class B".into(),
                scope: "B".into(),
            },
            DefContent {
                kind: DefKind::Function,
                lines: [3, 3],
                signature: "fn c".into(),
                scope: "c".into(),
            },
        ]);
        filter_file_defs(&mut fd, &[MatchMode::All], &[DefKind::Function], None);
        assert_eq!(fd.defs.len(), 2);
        assert!(fd.defs.iter().all(|d| d.kind == DefKind::Function));
    }

    #[test]
    fn filter_by_mode_exact() {
        let mut fd = make_fd(vec![
            DefContent {
                kind: DefKind::Function,
                lines: [1, 1],
                signature: "fn foo".into(),
                scope: "foo".into(),
            },
            DefContent {
                kind: DefKind::Function,
                lines: [2, 2],
                signature: "fn bar".into(),
                scope: "bar".into(),
            },
        ]);
        let mode = MatchMode::from_user_input("foo", false, false).unwrap();
        filter_file_defs(&mut fd, &[mode], &[DefKind::Function], None);
        assert_eq!(fd.defs.len(), 1);
        assert_eq!(fd.defs[0].scope, "foo");
    }

    #[test]
    fn filter_by_mode_with_scope() {
        let mut fd = make_fd(vec![
            DefContent {
                kind: DefKind::Function,
                lines: [1, 1],
                signature: "fn method".into(),
                scope: "MyClass.method".into(),
            },
            DefContent {
                kind: DefKind::Function,
                lines: [2, 2],
                signature: "fn other".into(),
                scope: "method".into(),
            },
        ]);
        let mode = MatchMode::from_user_input("method", false, false).unwrap();
        filter_file_defs(&mut fd, &[mode], DefKind::all(), None);
        assert_eq!(fd.defs.len(), 2);
    }

    #[test]
    fn filter_kind_and_mode_combined() {
        let mut fd = make_fd(vec![
            DefContent {
                kind: DefKind::Function,
                lines: [1, 1],
                signature: "fn foo".into(),
                scope: "foo".into(),
            },
            DefContent {
                kind: DefKind::Class,
                lines: [2, 2],
                signature: "class foo".into(),
                scope: "foo".into(),
            },
        ]);
        let mode = MatchMode::from_user_input("foo", false, false).unwrap();
        filter_file_defs(&mut fd, &[mode], &[DefKind::Function], None);
        assert_eq!(fd.defs.len(), 1);
        assert_eq!(fd.defs[0].kind, DefKind::Function);
    }

    #[test]
    fn filter_matches_qualified_name_from_scope() {
        let mut fd = make_fd(vec![
            DefContent {
                kind: DefKind::Function,
                lines: [10, 15],
                signature: "void Engine::start()".into(),
                scope: "Engine::start".into(),
            },
            DefContent {
                kind: DefKind::Function,
                lines: [1, 5],
                signature: "void run()".into(),
                scope: "run".into(),
            },
        ]);
        let mode = MatchMode::from_user_input("Engine::start", false, false).unwrap();
        filter_file_defs(&mut fd, &[mode], DefKind::all(), None);
        assert_eq!(fd.defs.len(), 1);
        assert_eq!(fd.defs[0].scope, "Engine::start");
    }

    #[test]
    fn filter_matches_short_name_from_qualified_scope() {
        let mut fd = make_fd(vec![DefContent {
            kind: DefKind::Function,
            lines: [10, 15],
            signature: "void Engine::start()".into(),
            scope: "Engine::start".into(),
        }]);
        let mode = MatchMode::from_user_input("start", false, false).unwrap();
        filter_file_defs(&mut fd, &[mode], DefKind::all(), None);
        assert_eq!(fd.defs.len(), 1);
    }

    #[test]
    fn filter_substring_matches_scope() {
        let mut fd = make_fd(vec![
            DefContent {
                kind: DefKind::Function,
                lines: [1, 1],
                signature: "fn method".into(),
                scope: "MyClass.method".into(),
            },
            DefContent {
                kind: DefKind::Function,
                lines: [2, 2],
                signature: "fn other".into(),
                scope: "OtherClass.other".into(),
            },
        ]);
        let mode = MatchMode::from_user_input("Class", false, false).unwrap();
        filter_file_defs(&mut fd, &[mode], DefKind::all(), None);
        assert_eq!(fd.defs.len(), 2);
    }

    // --- Multi-mode filter ---

    #[test]
    fn filter_multi_mode_literal_or() {
        let mut fd = make_fd(vec![
            DefContent {
                kind: DefKind::Function,
                lines: [1, 1],
                signature: "fn foo".into(),
                scope: "foo".into(),
            },
            DefContent {
                kind: DefKind::Function,
                lines: [2, 2],
                signature: "fn bar".into(),
                scope: "bar".into(),
            },
            DefContent {
                kind: DefKind::Function,
                lines: [3, 3],
                signature: "fn baz".into(),
                scope: "baz".into(),
            },
        ]);
        let modes = vec![
            MatchMode::from_user_input("foo", false, false).unwrap(),
            MatchMode::from_user_input("bar", false, false).unwrap(),
        ];
        filter_file_defs(&mut fd, &modes, DefKind::all(), None);
        assert_eq!(fd.defs.len(), 2);
        assert_eq!(fd.defs[0].scope, "foo");
        assert_eq!(fd.defs[1].scope, "bar");
    }

    #[test]
    fn filter_multi_mode_with_all_short_circuits() {
        let mut fd = make_fd(vec![
            DefContent {
                kind: DefKind::Function,
                lines: [1, 1],
                signature: "fn foo".into(),
                scope: "foo".into(),
            },
            DefContent {
                kind: DefKind::Function,
                lines: [2, 2],
                signature: "fn bar".into(),
                scope: "bar".into(),
            },
        ]);
        let modes = vec![
            MatchMode::from_user_input("nonexistent", false, false).unwrap(),
            MatchMode::All,
        ];
        filter_file_defs(&mut fd, &modes, DefKind::all(), None);
        assert_eq!(fd.defs.len(), 2);
    }

    #[test]
    fn filter_multi_mode_empty_matches_nothing() {
        let mut fd = make_fd(vec![DefContent {
            kind: DefKind::Function,
            lines: [1, 1],
            signature: "fn foo".into(),
            scope: "foo".into(),
        }]);
        let modes: Vec<MatchMode> = vec![];
        filter_file_defs(&mut fd, &modes, DefKind::all(), None);
        assert!(fd.defs.is_empty());
    }

    #[test]
    fn filter_multi_mode_mixed_literal_and_fuzzy() {
        let mut fd = make_fd(vec![
            DefContent {
                kind: DefKind::Function,
                lines: [1, 1],
                signature: "fn foo".into(),
                scope: "foo".into(),
            },
            DefContent {
                kind: DefKind::Function,
                lines: [2, 2],
                signature: "fn bar_baz".into(),
                scope: "bar_baz".into(),
            },
            DefContent {
                kind: DefKind::Function,
                lines: [3, 3],
                signature: "fn qux".into(),
                scope: "qux".into(),
            },
        ]);
        let regex_mode = MatchMode::from_user_input("bar.*", false, false).unwrap();
        let simple_mode = MatchMode::from_user_input("foo", false, false).unwrap();
        filter_file_defs(&mut fd, &[simple_mode, regex_mode], DefKind::all(), None);
        assert_eq!(fd.defs.len(), 2);
        assert_eq!(fd.defs[0].scope, "foo");
        assert_eq!(fd.defs[1].scope, "bar_baz");
    }

    // --- Scope depth filtering ---

    #[test]
    fn scope_depth_counts_dot_separators() {
        assert_eq!(scope_depth("MyClass"), 1);
        assert_eq!(scope_depth("MyClass.method"), 2);
        assert_eq!(scope_depth("Foo.bar.baz"), 3);
    }

    #[test]
    fn scope_depth_counts_double_colon_separators() {
        assert_eq!(scope_depth("foo"), 1);
        assert_eq!(scope_depth("Foo::bar"), 2);
        assert_eq!(scope_depth("A::B::C"), 3);
    }

    #[test]
    fn scope_depth_counts_backslash_separators() {
        assert_eq!(scope_depth("User"), 1);
        assert_eq!(scope_depth(r"App\Models\User"), 3);
    }

    #[test]
    fn filter_by_max_scope_depth() {
        let mut fd = make_fd(vec![
            DefContent {
                kind: DefKind::Function,
                lines: [1, 1],
                signature: "fn top".into(),
                scope: "top".into(),
            },
            DefContent {
                kind: DefKind::Function,
                lines: [2, 2],
                signature: "fn nested".into(),
                scope: "MyClass.nested".into(),
            },
            DefContent {
                kind: DefKind::Function,
                lines: [3, 3],
                signature: "fn deep".into(),
                scope: "A.B.C.deep".into(),
            },
        ]);
        filter_file_defs(&mut fd, &[MatchMode::All], DefKind::all(), Some(2));
        assert_eq!(fd.defs.len(), 2);
        assert!(fd.defs.iter().all(|d| scope_depth(&d.scope) <= 2));
    }

    #[test]
    fn filter_by_max_scope_depth_none_passes_all() {
        let mut fd = make_fd(vec![DefContent {
            kind: DefKind::Function,
            lines: [1, 1],
            signature: "fn a".into(),
            scope: "A.B.C.D".into(),
        }]);
        filter_file_defs(&mut fd, &[MatchMode::All], DefKind::all(), None);
        assert_eq!(fd.defs.len(), 1);
    }

    #[test]
    fn filter_by_max_scope_depth_one_keeps_only_toplevel() {
        let mut fd = make_fd(vec![
            DefContent {
                kind: DefKind::Function,
                lines: [1, 1],
                signature: "fn top".into(),
                scope: "top".into(),
            },
            DefContent {
                kind: DefKind::Function,
                lines: [2, 2],
                signature: "fn inner".into(),
                scope: "MyClass.inner".into(),
            },
        ]);
        filter_file_defs(&mut fd, &[MatchMode::All], DefKind::all(), Some(1));
        assert_eq!(fd.defs.len(), 1);
        assert_eq!(fd.defs[0].scope, "top");
    }

    // --- Cache integration ---

    #[test]
    fn cache_hit_produces_same_results_as_cache_miss() {
        let reg = build_registry();
        let modes = parse_mode("top_level_func");

        // First search: cache miss (populates cache)
        let results_miss = search(
            &modes,
            &[DefKind::Function],
            &[Path::new("tests/fixtures/python")],
            &[],
            &SearchOptions::default(),
            &reg,
        )
        .unwrap();
        assert!(!results_miss.definitions.is_empty());

        // Second search: should produce same results regardless of cache state
        let results_hit = search(
            &modes,
            &[DefKind::Function],
            &[Path::new("tests/fixtures/python")],
            &[],
            &SearchOptions::default(),
            &reg,
        )
        .unwrap();

        let miss_defs: Vec<_> = results_miss
            .definitions
            .iter()
            .flat_map(|fd| &fd.defs)
            .collect();
        let hit_defs: Vec<_> = results_hit
            .definitions
            .iter()
            .flat_map(|fd| &fd.defs)
            .collect();
        assert_eq!(miss_defs.len(), hit_defs.len());
        for (a, b) in miss_defs.iter().zip(hit_defs.iter()) {
            assert_eq!(a.kind, b.kind);
            assert_eq!(a.signature, b.signature);
            assert_eq!(a.lines, b.lines);
            assert_eq!(a.scope, b.scope);
        }
    }

    #[test]
    fn cache_stores_full_definitions() {
        let reg = build_registry();

        let modes_all = parse_mode("MyClass");
        let results_all = search(
            &modes_all,
            DefKind::all(),
            &[Path::new("tests/fixtures/python")],
            &[],
            &SearchOptions::default(),
            &reg,
        )
        .unwrap();

        let results_filtered = search(
            &modes_all,
            &[DefKind::Class],
            &[Path::new("tests/fixtures/python")],
            &[],
            &SearchOptions::default(),
            &reg,
        )
        .unwrap();

        let all_defs: Vec<_> = results_all
            .definitions
            .iter()
            .flat_map(|fd| &fd.defs)
            .collect();
        let filtered_defs: Vec<_> = results_filtered
            .definitions
            .iter()
            .flat_map(|fd| &fd.defs)
            .collect();
        assert!(filtered_defs.len() <= all_defs.len());
        assert!(filtered_defs.iter().all(|d| d.kind == DefKind::Class));
    }

    #[test]
    fn cache_files_written_to_disk_with_relative_path() {
        // Bug #1 regression test: cache must work when search path is relative.
        // Verify that cache.bin (v4 aggregated format) is written to .peek-cache/.
        let reg = build_registry();
        let modes = parse_mode("top_level_func");

        // Run search with relative path (typical CLI usage)
        let results = search(
            &modes,
            &[DefKind::Function],
            &[Path::new("tests/fixtures/python")],
            &[],
            &SearchOptions::default(),
            &reg,
        )
        .unwrap();
        assert!(!results.definitions.is_empty());

        // Verify v4 cache.bin was created (not old v3 files/ directory)
        let cache_bin = std::path::Path::new(".peek-cache/cache.bin");
        assert!(
            cache_bin.exists(),
            ".peek-cache/cache.bin should exist after search (v4 format)"
        );
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

    #[test]
    fn absolute_file_path_does_not_create_cache_under_file_dir() {
        // Bug #6 regression: searching an absolute file path outside project root
        // must not attempt to create .peek-cache inside the file's parent directory.
        let tmp = tempfile::tempdir().unwrap();
        let py_file = tmp.path().join("target_file.py");
        std::fs::write(&py_file, "def my_abs_func(): pass\n").unwrap();

        let reg = build_registry();
        let modes = parse_mode("my_abs_func");
        let results = search(
            &modes,
            &[DefKind::Function],
            &[&py_file],
            &[],
            &SearchOptions::default(),
            &reg,
        )
        .unwrap();

        // Should find the function
        assert!(!results.definitions.is_empty());

        // Should NOT create .peek-cache in the tempdir (file is outside project root)
        let cache_in_tmp = tmp.path().join(".peek-cache");
        assert!(
            !cache_in_tmp.exists(),
            ".peek-cache should not be created outside project root"
        );
    }
}
