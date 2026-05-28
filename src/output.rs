use crate::model::DefKind;
use crate::pipeline::SearchResult;
use serde::Serialize;
use std::borrow::Cow;
use std::fmt::Write;
use std::path::Path;

/// Maximum signature length for display. Truncated with " [truncated]" suffix if exceeded.
pub(crate) const MAX_SIGNATURE_LEN: usize = 256;

/// Convert a path to absolute: if already absolute, return as-is;
/// otherwise join with current directory.
pub(crate) fn absolutize(path: &Path) -> std::path::PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        match std::env::current_dir() {
            Ok(cwd) => cwd.join(path),
            Err(_) => path.to_path_buf(),
        }
    }
}

/// Format a single definition line: `file:range [kind/scope] sig[truncated]`.
/// Reused by both CLI output and MCP result formatting.
pub(crate) fn format_def_line(
    file: &str,
    def: &crate::model::DefContent,
    show_file: bool,
) -> String {
    let kind = def.kind.display_tag();
    let range = format_line_range(def.lines[0], def.lines[1]);
    let sig = truncate_str(&def.signature, MAX_SIGNATURE_LEN);
    let truncation = if def.signature.len() > MAX_SIGNATURE_LEN {
        " [truncated]"
    } else {
        ""
    };
    if show_file {
        format!(
            "{}:{} [{}/{}] {}{}",
            file, range, kind, def.scope, sig, truncation
        )
    } else {
        format!("{} [{}/{}] {}{}", range, kind, def.scope, sig, truncation)
    }
}

#[derive(Copy, Clone)]
pub enum OutputMode {
    Normal {
        survey: bool,
        no_signature: bool,
        no_filename: bool,
        heading: bool,
    },
    FilesOnly,
    Count {
        no_filename: bool,
    },
}

pub fn format_output(_name: &str, _path: &str, result: &SearchResult, mode: OutputMode) -> String {
    let mut out = String::new();
    let cwd = std::env::current_dir().unwrap_or_default();

    if !result.definitions.is_empty() {
        match mode {
            OutputMode::FilesOnly => {
                for fd in &result.definitions {
                    let _ = writeln!(out, "{}", relativize_path(&fd.file, &cwd));
                }
            }
            OutputMode::Count { no_filename } => {
                for fd in &result.definitions {
                    if no_filename {
                        let _ = writeln!(out, "{}", fd.defs.len());
                    } else {
                        let _ =
                            writeln!(out, "{}:{}", relativize_path(&fd.file, &cwd), fd.defs.len());
                    }
                }
            }
            OutputMode::Normal {
                survey,
                no_signature,
                no_filename,
                heading,
            } => {
                for (i, fd) in result.definitions.iter().enumerate() {
                    let file = relativize_path(&fd.file, &cwd);
                    if heading && !no_filename {
                        if i > 0 {
                            out.push('\n');
                        }
                        out.push_str(&file);
                        out.push('\n');
                    }
                    let suppress_file = no_filename || heading;
                    let mut max_end: u32 = 0;
                    for def in &fd.defs {
                        let is_contained = survey && def.lines[1] <= max_end;
                        if is_contained {
                            if !no_signature {
                                write_def_abbreviated(&mut out, def);
                                out.push('\n');
                            }
                            // no_signature + contained: omit entirely
                        } else {
                            write_def(&mut out, &file, def, no_signature, suppress_file);
                            out.push('\n');
                        }
                        if def.lines[1] > max_end {
                            max_end = def.lines[1];
                        }
                    }
                }
            }
        }
        out.pop();
    }

    out
}

// --- JSON output ---

#[derive(Serialize)]
#[serde(tag = "type", content = "data", rename_all = "lowercase")]
enum JsonMessage {
    Begin {
        path: String,
    },
    Match {
        path: String,
        line_start: u32,
        line_end: u32,
        kind: String,
        scope: String,
        signature: String,
    },
    End {
        path: String,
        matched: usize,
    },
    Summary {
        matched: usize,
        files: usize,
        errors: usize,
    },
}

pub fn format_json_output(result: &SearchResult, mode: OutputMode) -> String {
    let cwd = std::env::current_dir().unwrap_or_default();
    let mut out = String::new();
    let mut total_matched = 0usize;
    let serialize = |msg: &JsonMessage| -> String {
        serde_json::to_string(msg).expect("JSON serialization of JsonMessage should never fail")
    };

    for fd in &result.definitions {
        let path = relativize_path(&fd.file, &cwd).into_owned();

        out.push_str(&serialize(&JsonMessage::Begin { path: path.clone() }));
        out.push('\n');

        match mode {
            OutputMode::Normal { .. } => {
                for def in &fd.defs {
                    let sig = truncate_str(&def.signature, MAX_SIGNATURE_LEN);
                    let signature = if def.signature.len() > MAX_SIGNATURE_LEN {
                        format!("{sig} [truncated]")
                    } else {
                        sig.to_string()
                    };
                    out.push_str(&serialize(&JsonMessage::Match {
                        path: path.clone(),
                        line_start: def.lines[0],
                        line_end: def.lines[1],
                        kind: def.kind.display_tag().to_string(),
                        scope: def.scope.clone(),
                        signature,
                    }));
                    out.push('\n');
                }
            }
            OutputMode::FilesOnly | OutputMode::Count { .. } => {}
        }

        total_matched += fd.defs.len();
        out.push_str(&serialize(&JsonMessage::End {
            path,
            matched: fd.defs.len(),
        }));
        out.push('\n');
    }

    let errors = result.read_errors.len() + result.parse_failures.len();
    out.push_str(&serialize(&JsonMessage::Summary {
        matched: total_matched,
        files: result.definitions.len(),
        errors,
    }));

    out
}

/// Convert an absolute path to a relative path by stripping the base directory prefix.
/// Falls back to the original path if stripping fails (e.g., different drive on Windows).
/// On Windows, backslashes are replaced with forward slashes for consistent cross-platform output.
pub(crate) fn relativize_path<'a>(path: &'a Path, base: &Path) -> Cow<'a, str> {
    let lossy = match path.strip_prefix(base) {
        Ok(relative) => relative.to_string_lossy(),
        Err(_) => path.to_string_lossy(),
    };
    if cfg!(windows) && lossy.contains('\\') {
        let mut replaced = lossy.into_owned();
        // Safety: replacing ASCII \ (0x5C) with ASCII / (0x2F), both single-byte,
        // preserves UTF-8 validity.
        unsafe {
            for b in replaced.as_bytes_mut() {
                if *b == b'\\' {
                    *b = b'/';
                }
            }
        }
        Cow::Owned(replaced)
    } else {
        lossy
    }
}

/// Format: `file:start-end [kind/scope] signature` (or without signature/filename).
/// When start == end, the range collapses to a single line number (e.g., `15` instead of `15-15`).
fn write_def(
    out: &mut String,
    file: &str,
    def: &crate::model::DefContent,
    no_signature: bool,
    no_filename: bool,
) {
    let kind = def.kind.display_tag();
    let range = format_line_range(def.lines[0], def.lines[1]);
    if no_signature {
        if no_filename {
            write!(out, "{} [{}/{}]", range, kind, def.scope).unwrap();
        } else {
            write!(out, "{}:{} [{}/{}]", file, range, kind, def.scope).unwrap();
        }
    } else {
        let sig = truncate_str(&def.signature, MAX_SIGNATURE_LEN);
        let truncation = if def.signature.len() > MAX_SIGNATURE_LEN {
            " [truncated]"
        } else {
            ""
        };
        if no_filename {
            write!(
                out,
                "{} [{}/{}] {}{}",
                range, kind, def.scope, sig, truncation
            )
            .unwrap();
        } else {
            write!(
                out,
                "{}:{} [{}/{}] {}{}",
                file, range, kind, def.scope, sig, truncation
            )
            .unwrap();
        }
    }
}

/// Abbreviated format for survey mode: `start-end signature` or `line signature`.
fn write_def_abbreviated(out: &mut String, def: &crate::model::DefContent) {
    let range = format_line_range(def.lines[0], def.lines[1]);
    let sig = truncate_str(&def.signature, MAX_SIGNATURE_LEN);
    let truncation = if def.signature.len() > MAX_SIGNATURE_LEN {
        " [truncated]"
    } else {
        ""
    };
    write!(out, "{} {}{}", range, sig, truncation).unwrap();
}

pub(crate) fn format_line_range(start: u32, end: u32) -> String {
    if start == end {
        start.to_string()
    } else {
        format!("{start}-{end}")
    }
}

pub(crate) fn truncate_str(s: &str, max_len: usize) -> &str {
    if s.len() <= max_len {
        s
    } else {
        let mut end = max_len;
        while end > 0 && !s.is_char_boundary(end) {
            end -= 1;
        }
        &s[..end]
    }
}

/// Remove Getter/Setter entries that share the same `line_start` and `scope` as a
/// Property or Subscript entry within each `FileDefs`. When `-k getter`/`-k setter`
/// is used, Property/Subscript is absent from results so no dedup occurs.
pub fn dedup_accessors(definitions: &mut [crate::model::FileDefs]) {
    for fd in definitions.iter_mut() {
        let anchor_lines: std::collections::HashSet<(u32, String)> = fd
            .defs
            .iter()
            .filter(|d| matches!(d.kind, DefKind::Property | DefKind::Subscript))
            .map(|d| (d.lines[0], d.scope.clone()))
            .collect();
        fd.defs.retain(|d| {
            if !matches!(d.kind, DefKind::Getter | DefKind::Setter) {
                return true;
            }
            !anchor_lines.contains(&(d.lines[0], d.scope.clone()))
        });
    }
}

/// Write non-fatal errors to the given writer (typically stderr).
/// Each error is printed as `peek: {path}: {message}` on a separate line.
/// When `suppress` is true, no output is produced (but exit code is still affected).
pub fn write_errors<W: std::io::Write>(wtr: &mut W, result: &SearchResult, suppress: bool) {
    if suppress {
        return;
    }
    let cwd = std::env::current_dir().unwrap_or_default();
    for err in &result.read_errors {
        let path = relativize_path(&err.path, &cwd);
        let _ = writeln!(wtr, "peek: {}: {}", path, err.message);
    }
    for err in &result.parse_failures {
        let path = relativize_path(&err.path, &cwd);
        let _ = writeln!(wtr, "peek: {}: {}", path, err.message);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{DefContent, DefKind, FileDefs};
    use crate::pipeline::FileError;
    use std::path::PathBuf;

    fn make_fd(file: &str, defs: Vec<DefContent>) -> FileDefs {
        FileDefs {
            file: PathBuf::from(file),
            defs,
        }
    }

    fn make_def(kind: DefKind, sig: &str, start: u32, end: u32, scope: &str) -> DefContent {
        DefContent {
            kind,
            lines: [start, end],
            signature: sig.to_string(),
            scope: scope.to_string(),
        }
    }

    fn parse_json_lines(output: &str) -> Vec<serde_json::Value> {
        output
            .lines()
            .map(serde_json::from_str)
            .collect::<Result<_, _>>()
            .expect("valid JSON lines")
    }

    #[test]
    fn format_results_with_parse_failures() {
        let result = SearchResult {
            definitions: vec![make_fd(
                "foo.py",
                vec![make_def(DefKind::Class, "class Foo", 1, 5, "Foo")],
            )],
            read_errors: vec![],
            parse_failures: vec![
                FileError {
                    path: PathBuf::from("src/broken.py"),
                    message: "parse failure".into(),
                },
                FileError {
                    path: PathBuf::from("src/corrupted.py"),
                    message: "parse failure".into(),
                },
            ],
        };
        let output = format_output(
            "Foo",
            "src/",
            &result,
            OutputMode::Normal {
                survey: false,
                no_signature: false,
                no_filename: false,
                heading: false,
            },
        );
        // Errors should NOT appear in stdout output
        assert!(!output.contains("failed to parse"));
        assert!(!output.contains("src/broken.py"));
        let lines: Vec<&str> = output.lines().collect();
        assert_eq!(lines.len(), 1);
    }

    #[test]
    fn format_results_with_read_errors() {
        let result = SearchResult {
            definitions: vec![make_fd(
                "foo.py",
                vec![make_def(DefKind::Class, "class Foo", 1, 5, "Foo")],
            )],
            read_errors: vec![
                FileError {
                    path: PathBuf::from("src/missing.py"),
                    message: "permission denied".into(),
                },
                FileError {
                    path: PathBuf::from("src/no_access.py"),
                    message: "permission denied".into(),
                },
            ],
            parse_failures: vec![],
        };
        let output = format_output(
            "Foo",
            "src/",
            &result,
            OutputMode::Normal {
                survey: false,
                no_signature: false,
                no_filename: false,
                heading: false,
            },
        );
        // Errors should NOT appear in stdout output
        assert!(!output.contains("failed to read"));
        assert!(!output.contains("src/missing.py"));
        let lines: Vec<&str> = output.lines().collect();
        assert_eq!(lines.len(), 1);
    }

    #[test]
    fn no_signature_output_skips_all_sigs() {
        let result = SearchResult {
            definitions: vec![make_fd(
                "foo.py",
                vec![
                    make_def(DefKind::Class, "class Foo", 1, 5, "Foo"),
                    make_def(DefKind::Function, "def bar()", 10, 20, "Foo.bar"),
                ],
            )],
            read_errors: vec![],
            parse_failures: vec![],
        };
        let output = format_output(
            "Foo",
            "src/",
            &result,
            OutputMode::Normal {
                survey: false,
                no_signature: true,
                no_filename: false,
                heading: false,
            },
        );
        let lines: Vec<&str> = output.lines().collect();
        assert_eq!(lines[0], "foo.py:1-5 [class/Foo]");
        assert_eq!(lines[1], "foo.py:10-20 [function/Foo.bar]");
    }

    #[test]
    fn files_only_mode_single_file() {
        let result = SearchResult {
            definitions: vec![make_fd(
                "foo.py",
                vec![
                    make_def(DefKind::Class, "class Foo", 1, 5, "Foo"),
                    make_def(DefKind::Function, "def bar()", 10, 20, "Foo.bar"),
                ],
            )],
            read_errors: vec![],
            parse_failures: vec![],
        };
        let output = format_output("Foo", ".", &result, OutputMode::FilesOnly);
        let lines: Vec<&str> = output.lines().collect();
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0], "foo.py");
    }

    #[test]
    fn files_only_empty_result() {
        let result = SearchResult {
            definitions: vec![],
            read_errors: vec![],
            parse_failures: vec![],
        };
        let output = format_output("X", ".", &result, OutputMode::FilesOnly);
        assert!(output.is_empty());
    }

    #[test]
    fn count_mode_empty_result() {
        let result = SearchResult {
            definitions: vec![],
            read_errors: vec![],
            parse_failures: vec![],
        };
        let output = format_output("X", ".", &result, OutputMode::Count { no_filename: false });
        assert!(output.is_empty());
    }

    // --- relativize_path ---

    #[test]
    fn relativize_preserves_non_cwd_absolute_path() {
        let cwd = std::env::current_dir().unwrap();
        // Path outside CWD — strip_prefix fails, full path kept with / normalization on Windows
        let outside = cwd
            .parent()
            .unwrap_or(Path::new("/"))
            .join("other_project")
            .join("file.rs");
        let result = relativize_path(&outside, &cwd);
        // On Windows, backslashes are normalized to forward slashes
        let expected = outside.to_string_lossy().replace('\\', "/");
        assert_eq!(result, expected);
    }

    // --- JSON output mode ---

    #[test]
    fn json_files_only_mode() {
        let result = SearchResult {
            definitions: vec![
                make_fd(
                    "src/a.rs",
                    vec![make_def(DefKind::Function, "fn foo()", 1, 5, "foo")],
                ),
                make_fd(
                    "src/b.rs",
                    vec![make_def(DefKind::Class, "struct Bar", 10, 20, "Bar")],
                ),
            ],
            read_errors: vec![],
            parse_failures: vec![],
        };
        let output = format_json_output(&result, OutputMode::FilesOnly);
        let messages = parse_json_lines(&output);

        // begin + end per file + summary (no match messages)
        assert_eq!(messages.len(), 5);
        assert_eq!(messages[0]["type"], "begin");
        assert_eq!(messages[1]["type"], "end");
        // No match messages in files-only mode
        assert!(messages.iter().all(|m| m["type"] != "match"));
        assert_eq!(messages[4]["type"], "summary");
    }

    #[test]
    fn json_always_includes_signature() {
        let result = SearchResult {
            definitions: vec![make_fd(
                "src/a.rs",
                vec![make_def(DefKind::Function, "fn foo()", 1, 5, "foo")],
            )],
            read_errors: vec![],
            parse_failures: vec![],
        };
        // Even with no_signature=true, JSON output includes signature
        let output = format_json_output(
            &result,
            OutputMode::Normal {
                survey: false,
                no_signature: true,
                no_filename: false,
                heading: false,
            },
        );
        let messages = parse_json_lines(&output);

        let match_msg = messages.iter().find(|m| m["type"] == "match").unwrap();
        assert!(match_msg["data"]["signature"].is_string());
        assert_eq!(match_msg["data"]["signature"], "fn foo()");
    }

    // --- write_errors (stderr output) ---

    #[test]
    fn write_errors_suppress_with_flag() {
        let result = SearchResult {
            definitions: vec![],
            read_errors: vec![FileError {
                path: PathBuf::from("src/missing.py"),
                message: "not found".into(),
            }],
            parse_failures: vec![],
        };
        let mut buf = Vec::new();
        write_errors(&mut buf, &result, true);
        assert!(buf.is_empty());
    }

    #[test]
    fn write_errors_empty() {
        let result = SearchResult {
            definitions: vec![],
            read_errors: vec![],
            parse_failures: vec![],
        };
        let mut buf = Vec::new();
        write_errors(&mut buf, &result, false);
        assert!(buf.is_empty());
    }

    #[test]
    fn count_mode_no_filename() {
        let result = SearchResult {
            definitions: vec![make_fd(
                "src/models.py",
                vec![
                    make_def(DefKind::Class, "class Foo", 1, 5, "Foo"),
                    make_def(DefKind::Function, "def bar()", 10, 20, "Foo.bar"),
                ],
            )],
            read_errors: vec![],
            parse_failures: vec![],
        };
        let output = format_output("foo", ".", &result, OutputMode::Count { no_filename: true });
        assert_eq!(output, "2");
    }

    #[test]
    fn survey_with_no_signature_omits_contained() {
        // When --no-signature, contained defs are omitted entirely
        let result = SearchResult {
            definitions: vec![make_fd(
                "src/pattern.rs",
                vec![
                    make_def(DefKind::Enum, "pub enum Foo", 1, 10, "Foo"),
                    make_def(DefKind::Variant, "A", 3, 3, "Foo.A"),
                    make_def(DefKind::Variant, "B", 4, 4, "Foo.B"),
                    make_def(DefKind::Function, "fn bar()", 15, 20, "bar"),
                ],
            )],
            read_errors: vec![],
            parse_failures: vec![],
        };
        let output = format_output(
            "*",
            "src/",
            &result,
            OutputMode::Normal {
                survey: true,
                no_signature: true,
                no_filename: false,
                heading: false,
            },
        );
        let lines: Vec<&str> = output.lines().collect();
        // Only top-level defs shown
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0], "src/pattern.rs:1-10 [enum/Foo]");
        assert_eq!(lines[1], "src/pattern.rs:15-20 [function/bar]");
    }

    #[test]
    fn survey_containment_is_per_file() {
        // Definitions in different files don't affect each other
        let result = SearchResult {
            definitions: vec![
                make_fd(
                    "src/a.rs",
                    vec![make_def(DefKind::Enum, "pub enum E", 1, 10, "E")],
                ),
                make_fd(
                    "src/b.rs",
                    // Line 5 is NOT contained in src/a.rs's 1-10 (different file)
                    vec![make_def(DefKind::Function, "fn foo()", 5, 5, "foo")],
                ),
            ],
            read_errors: vec![],
            parse_failures: vec![],
        };
        let output = format_output(
            "*",
            "src/",
            &result,
            OutputMode::Normal {
                survey: true,
                no_signature: false,
                no_filename: false,
                heading: false,
            },
        );
        let lines: Vec<&str> = output.lines().collect();
        assert_eq!(lines.len(), 2);
        // Both are top-level in their respective files
        assert_eq!(lines[0], "src/a.rs:1-10 [enum/E] pub enum E");
        assert_eq!(lines[1], "src/b.rs:5 [function/foo] fn foo()");
    }

    // --- dedup_accessors ---

    #[test]
    fn dedup_removes_getter_same_line_as_property() {
        let mut defs = vec![make_fd(
            "test.cs",
            vec![
                make_def(DefKind::Property, "int X { get; }", 10, 10, "Foo.X"),
                make_def(DefKind::Getter, "int X { get; }", 10, 10, "Foo.X"),
            ],
        )];
        super::dedup_accessors(&mut defs);
        assert_eq!(defs[0].defs.len(), 1);
        assert_eq!(defs[0].defs[0].kind, DefKind::Property);
    }

    #[test]
    fn dedup_removes_setter_same_line_as_property() {
        let mut defs = vec![make_fd(
            "test.cs",
            vec![
                make_def(DefKind::Property, "int X { set; }", 10, 10, "Foo.X"),
                make_def(DefKind::Setter, "int X { set; }", 10, 10, "Foo.X"),
            ],
        )];
        super::dedup_accessors(&mut defs);
        assert_eq!(defs[0].defs.len(), 1);
        assert_eq!(defs[0].defs[0].kind, DefKind::Property);
    }

    #[test]
    fn dedup_removes_accessor_same_line_as_subscript() {
        let mut defs = vec![make_fd(
            "test.cs",
            vec![
                make_def(DefKind::Subscript, "this[int i]", 10, 15, "Foo.this"),
                make_def(DefKind::Getter, "get", 10, 10, "Foo.this"),
                make_def(DefKind::Setter, "set", 10, 10, "Foo.this"),
            ],
        )];
        super::dedup_accessors(&mut defs);
        assert_eq!(defs[0].defs.len(), 1);
        assert_eq!(defs[0].defs[0].kind, DefKind::Subscript);
    }

    #[test]
    fn dedup_keeps_accessor_with_different_scope() {
        let mut defs = vec![make_fd(
            "test.cs",
            vec![
                make_def(DefKind::Property, "int X { get; }", 10, 10, "Foo.X"),
                make_def(DefKind::Getter, "get;", 10, 10, "Bar.X"),
            ],
        )];
        super::dedup_accessors(&mut defs);
        assert_eq!(defs[0].defs.len(), 2);
    }
}
