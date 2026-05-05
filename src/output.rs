use crate::pipeline::SearchResult;
use serde::Serialize;
use std::borrow::Cow;
use std::fmt::Write;
use std::path::Path;

/// Maximum signature length for display. Truncated with " [truncated]" suffix if exceeded.
const MAX_SIGNATURE_LEN: usize = 256;

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
fn relativize_path<'a>(path: &'a Path, base: &Path) -> Cow<'a, str> {
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

fn format_line_range(start: u32, end: u32) -> String {
    if start == end {
        start.to_string()
    } else {
        format!("{start}-{end}")
    }
}

fn truncate_str(s: &str, max_len: usize) -> &str {
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
    fn format_results_with_definitions() {
        let result = SearchResult {
            definitions: vec![
                make_fd(
                    "src/models.py",
                    vec![make_def(
                        DefKind::Class,
                        "class MyClass(Base)",
                        42,
                        85,
                        "MyClass",
                    )],
                ),
                make_fd(
                    "src/handler.py",
                    vec![make_def(
                        DefKind::Function,
                        "def process_data(items)",
                        15,
                        30,
                        "process_data",
                    )],
                ),
            ],
            read_errors: vec![],
            parse_failures: vec![],
        };
        let output = format_output(
            "MyClass",
            "src/",
            &result,
            OutputMode::Normal {
                survey: false,
                no_signature: false,
                no_filename: false,
                heading: false,
            },
        );
        let lines: Vec<&str> = output.lines().collect();
        assert_eq!(
            lines[0],
            "src/models.py:42-85 [class/MyClass] class MyClass(Base)"
        );
        assert_eq!(
            lines[1],
            "src/handler.py:15-30 [function/process_data] def process_data(items)"
        );
        assert_eq!(lines.len(), 2);
    }

    #[test]
    fn format_results_no_summary_line() {
        let result = SearchResult {
            definitions: vec![make_fd(
                "foo.py",
                vec![make_def(DefKind::Class, "class Foo", 1, 5, "Foo")],
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
                no_signature: false,
                no_filename: false,
                heading: false,
            },
        );
        assert!(!output.contains("Found"));
        assert!(!output.contains("definition"));
    }

    #[test]
    fn format_results_empty() {
        let result = SearchResult {
            definitions: vec![],
            read_errors: vec![],
            parse_failures: vec![],
        };
        let output = format_output(
            "NonExist",
            ".",
            &result,
            OutputMode::Normal {
                survey: false,
                no_signature: false,
                no_filename: false,
                heading: false,
            },
        );
        assert!(
            output.is_empty(),
            "expected silent output on no match, got: {output}"
        );
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
    fn format_results_no_failures_omits_line() {
        let result = SearchResult {
            definitions: vec![make_fd(
                "foo.py",
                vec![make_def(DefKind::Function, "def foo()", 1, 1, "foo")],
            )],
            read_errors: vec![],
            parse_failures: vec![],
        };
        let output = format_output(
            "foo",
            ".",
            &result,
            OutputMode::Normal {
                survey: false,
                no_signature: false,
                no_filename: false,
                heading: false,
            },
        );
        assert!(!output.contains("failed to parse"));
        assert!(!output.contains("failed to read"));
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
    fn top_level_definition_format() {
        let result = SearchResult {
            definitions: vec![make_fd(
                "src/abc.py",
                vec![make_def(
                    DefKind::Function,
                    "def process() -> bool",
                    45,
                    62,
                    "process",
                )],
            )],
            read_errors: vec![],
            parse_failures: vec![],
        };
        let output = format_output(
            "process",
            ".",
            &result,
            OutputMode::Normal {
                survey: false,
                no_signature: false,
                no_filename: false,
                heading: false,
            },
        );
        assert_eq!(
            output,
            "src/abc.py:45-62 [function/process] def process() -> bool"
        );
    }

    #[test]
    fn nested_definition_format() {
        let result = SearchResult {
            definitions: vec![make_fd(
                "src/handler.py",
                vec![make_def(
                    DefKind::Function,
                    "def run(self)",
                    10,
                    20,
                    "Handler.run",
                )],
            )],
            read_errors: vec![],
            parse_failures: vec![],
        };
        let output = format_output(
            "run",
            ".",
            &result,
            OutputMode::Normal {
                survey: false,
                no_signature: false,
                no_filename: false,
                heading: false,
            },
        );
        assert_eq!(
            output,
            "src/handler.py:10-20 [function/Handler.run] def run(self)"
        );
    }

    #[test]
    fn no_signature_mode() {
        let result = SearchResult {
            definitions: vec![make_fd(
                "src/abc.py",
                vec![make_def(
                    DefKind::Function,
                    "def process() -> bool",
                    45,
                    62,
                    "process",
                )],
            )],
            read_errors: vec![],
            parse_failures: vec![],
        };
        let output = format_output(
            "process",
            ".",
            &result,
            OutputMode::Normal {
                survey: false,
                no_signature: true,
                no_filename: false,
                heading: false,
            },
        );
        assert_eq!(output, "src/abc.py:45-62 [function/process]");
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

    // --- OutputMode::FilesOnly ---

    #[test]
    fn files_only_mode() {
        let result = SearchResult {
            definitions: vec![
                make_fd(
                    "src/models.py",
                    vec![make_def(DefKind::Class, "class MyClass", 42, 85, "MyClass")],
                ),
                make_fd(
                    "src/handler.py",
                    vec![make_def(
                        DefKind::Function,
                        "def process()",
                        15,
                        30,
                        "process",
                    )],
                ),
            ],
            read_errors: vec![],
            parse_failures: vec![],
        };
        let output = format_output("foo", ".", &result, OutputMode::FilesOnly);
        let lines: Vec<&str> = output.lines().collect();
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0], "src/models.py");
        assert_eq!(lines[1], "src/handler.py");
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

    // --- OutputMode::Count ---

    #[test]
    fn count_mode() {
        let result = SearchResult {
            definitions: vec![
                make_fd(
                    "src/models.py",
                    vec![
                        make_def(DefKind::Class, "class Foo", 1, 5, "Foo"),
                        make_def(DefKind::Function, "def bar()", 10, 20, "Foo.bar"),
                    ],
                ),
                make_fd(
                    "src/handler.py",
                    vec![make_def(
                        DefKind::Function,
                        "def process()",
                        15,
                        30,
                        "process",
                    )],
                ),
            ],
            read_errors: vec![],
            parse_failures: vec![],
        };
        let output = format_output(
            "foo",
            ".",
            &result,
            OutputMode::Count { no_filename: false },
        );
        let lines: Vec<&str> = output.lines().collect();
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0], "src/models.py:2");
        assert_eq!(lines[1], "src/handler.py:1");
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

    // --- Signature truncation ---

    #[test]
    fn short_signature_not_truncated() {
        let short_sig = "fn foo()".to_string();
        let result = SearchResult {
            definitions: vec![make_fd(
                "f.rs",
                vec![DefContent {
                    kind: DefKind::Function,
                    lines: [1, 1],
                    signature: short_sig,
                    scope: "foo".into(),
                }],
            )],
            read_errors: vec![],
            parse_failures: vec![],
        };
        let output = format_output(
            "foo",
            ".",
            &result,
            OutputMode::Normal {
                survey: false,
                no_signature: false,
                no_filename: false,
                heading: false,
            },
        );
        assert!(output.contains("fn foo()"));
        assert!(!output.contains(" [truncated]"));
    }

    #[test]
    fn long_signature_truncated() {
        let long_sig = "x".repeat(300);
        let result = SearchResult {
            definitions: vec![make_fd(
                "f.rs",
                vec![DefContent {
                    kind: DefKind::Function,
                    lines: [1, 1],
                    signature: long_sig,
                    scope: "foo".into(),
                }],
            )],
            read_errors: vec![],
            parse_failures: vec![],
        };
        let output = format_output(
            "foo",
            ".",
            &result,
            OutputMode::Normal {
                survey: false,
                no_signature: false,
                no_filename: false,
                heading: false,
            },
        );
        assert!(output.ends_with(" [truncated]"));
        // Output should be shorter than the original 300-char signature
        let sig_part = output.split(']').next_back().unwrap().trim();
        assert!(sig_part.len() < 300);
        assert!(sig_part.len() <= MAX_SIGNATURE_LEN + 12); // +12 for " [truncated]"
    }

    #[test]
    fn exact_max_len_signature_not_truncated() {
        let exact_sig = "x".repeat(MAX_SIGNATURE_LEN);
        let result = SearchResult {
            definitions: vec![make_fd(
                "f.rs",
                vec![DefContent {
                    kind: DefKind::Function,
                    lines: [1, 1],
                    signature: exact_sig,
                    scope: "foo".into(),
                }],
            )],
            read_errors: vec![],
            parse_failures: vec![],
        };
        let output = format_output(
            "foo",
            ".",
            &result,
            OutputMode::Normal {
                survey: false,
                no_signature: false,
                no_filename: false,
                heading: false,
            },
        );
        assert!(!output.contains(" [truncated]"));
    }

    #[test]
    fn one_over_max_len_signature_truncated() {
        let sig = "x".repeat(MAX_SIGNATURE_LEN + 1);
        let result = SearchResult {
            definitions: vec![make_fd(
                "f.rs",
                vec![DefContent {
                    kind: DefKind::Function,
                    lines: [1, 1],
                    signature: sig,
                    scope: "foo".into(),
                }],
            )],
            read_errors: vec![],
            parse_failures: vec![],
        };
        let output = format_output(
            "foo",
            ".",
            &result,
            OutputMode::Normal {
                survey: false,
                no_signature: false,
                no_filename: false,
                heading: false,
            },
        );
        assert!(output.ends_with(" [truncated]"));
    }

    // --- relativize_path ---

    #[test]
    fn relativize_strips_cwd_prefix_from_absolute_path() {
        let cwd = std::env::current_dir().unwrap();
        let abs_path = cwd.join("src").join("main.rs");
        let result = relativize_path(&abs_path, &cwd);
        assert_eq!(result, "src/main.rs");
    }

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

    #[test]
    fn relativize_preserves_relative_path() {
        let cwd = std::env::current_dir().unwrap();
        let rel = PathBuf::from("src/main.rs");
        let result = relativize_path(&rel, &cwd);
        assert_eq!(result, rel.to_string_lossy());
    }

    #[test]
    fn relativize_normalizes_backslashes_on_windows() {
        let cwd = std::env::current_dir().unwrap();
        let abs_path = cwd.join("src").join("main.rs");
        let result = relativize_path(&abs_path, &cwd);
        assert_eq!(result, "src/main.rs");
        if cfg!(windows) {
            assert!(
                !result.contains('\\'),
                "expected no backslashes on Windows, got: {}",
                result
            );
        }
    }

    #[test]
    fn relativize_strips_cwd_and_uses_forward_slash() {
        let cwd = std::env::current_dir().unwrap();
        let abs_path = cwd.join("src").join("main.rs");
        let result = relativize_path(&abs_path, &cwd);
        assert_eq!(result, "src/main.rs");
    }

    // --- format_output with absolute paths ---

    #[test]
    fn format_output_converts_absolute_path_to_relative() {
        let cwd = std::env::current_dir().unwrap();
        let abs_file = cwd.join("src").join("models.py");
        let result = SearchResult {
            definitions: vec![FileDefs {
                file: abs_file,
                defs: vec![make_def(DefKind::Class, "class MyClass", 42, 85, "MyClass")],
            }],
            read_errors: vec![],
            parse_failures: vec![],
        };
        let output = format_output(
            "MyClass",
            "src/",
            &result,
            OutputMode::Normal {
                survey: false,
                no_signature: false,
                no_filename: false,
                heading: false,
            },
        );
        assert!(
            output.starts_with("src/models.py"),
            "expected relative path, got: {output}"
        );
    }

    #[test]
    fn files_only_mode_with_absolute_paths() {
        let cwd = std::env::current_dir().unwrap();
        let abs_file = cwd.join("src").join("models.py");
        let result = SearchResult {
            definitions: vec![FileDefs {
                file: abs_file,
                defs: vec![make_def(DefKind::Class, "class MyClass", 42, 85, "MyClass")],
            }],
            read_errors: vec![],
            parse_failures: vec![],
        };
        let output = format_output("MyClass", ".", &result, OutputMode::FilesOnly);
        assert_eq!(output, "src/models.py");
    }

    #[test]
    fn count_mode_with_absolute_paths() {
        let cwd = std::env::current_dir().unwrap();
        let abs_file = cwd.join("src").join("models.py");
        let result = SearchResult {
            definitions: vec![FileDefs {
                file: abs_file,
                defs: vec![
                    make_def(DefKind::Class, "class Foo", 1, 5, "Foo"),
                    make_def(DefKind::Function, "def bar()", 10, 20, "Foo.bar"),
                ],
            }],
            read_errors: vec![],
            parse_failures: vec![],
        };
        let output = format_output(
            "Foo",
            ".",
            &result,
            OutputMode::Count { no_filename: false },
        );
        assert_eq!(output, "src/models.py:2");
    }

    // --- JSON output mode ---

    #[test]
    fn json_normal_mode_single_file() {
        let result = SearchResult {
            definitions: vec![make_fd(
                "src/main.rs",
                vec![
                    make_def(DefKind::Function, "fn main()", 1, 10, "main"),
                    make_def(DefKind::Function, "fn helper()", 15, 20, "helper"),
                ],
            )],
            read_errors: vec![],
            parse_failures: vec![],
        };
        let output = format_json_output(
            &result,
            OutputMode::Normal {
                survey: false,
                no_signature: false,
                no_filename: false,
                heading: false,
            },
        );
        let messages = parse_json_lines(&output);

        assert_eq!(messages.len(), 5); // begin + 2 match + end + summary
        assert_eq!(messages[0]["type"], "begin");
        assert_eq!(messages[0]["data"]["path"], "src/main.rs");
        assert_eq!(messages[1]["type"], "match");
        assert_eq!(messages[1]["data"]["kind"], "function");
        assert_eq!(messages[1]["data"]["line_start"], 1);
        assert_eq!(messages[1]["data"]["line_end"], 10);
        assert_eq!(messages[1]["data"]["scope"], "main");
        assert_eq!(messages[1]["data"]["signature"], "fn main()");
        assert_eq!(messages[2]["type"], "match");
        assert_eq!(messages[2]["data"]["scope"], "helper");
        assert_eq!(messages[3]["type"], "end");
        assert_eq!(messages[3]["data"]["matched"], 2);
    }

    #[test]
    fn json_normal_mode_multiple_files() {
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
        let output = format_json_output(
            &result,
            OutputMode::Normal {
                survey: false,
                no_signature: false,
                no_filename: false,
                heading: false,
            },
        );
        let messages = parse_json_lines(&output);

        // begin + match + end (file a) + begin + match + end (file b) + summary
        assert_eq!(messages.len(), 7);
        assert_eq!(messages[0]["type"], "begin");
        assert_eq!(messages[0]["data"]["path"], "src/a.rs");
        assert_eq!(messages[1]["type"], "match");
        assert_eq!(messages[2]["type"], "end");
        assert_eq!(messages[2]["data"]["matched"], 1);
        assert_eq!(messages[3]["type"], "begin");
        assert_eq!(messages[3]["data"]["path"], "src/b.rs");
        assert_eq!(messages[4]["type"], "match");
        assert_eq!(messages[4]["data"]["kind"], "class");
        assert_eq!(messages[5]["type"], "end");
        assert_eq!(messages[6]["type"], "summary");
        assert_eq!(messages[6]["data"]["matched"], 2);
        assert_eq!(messages[6]["data"]["files"], 2);
        assert_eq!(messages[6]["data"]["errors"], 0);
    }

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
    fn json_count_mode() {
        let result = SearchResult {
            definitions: vec![
                make_fd(
                    "src/a.rs",
                    vec![
                        make_def(DefKind::Function, "fn foo()", 1, 5, "foo"),
                        make_def(DefKind::Function, "fn bar()", 10, 15, "bar"),
                    ],
                ),
                make_fd(
                    "src/b.rs",
                    vec![make_def(DefKind::Class, "struct Baz", 1, 10, "Baz")],
                ),
            ],
            read_errors: vec![],
            parse_failures: vec![],
        };
        let output = format_json_output(&result, OutputMode::Count { no_filename: false });
        let messages = parse_json_lines(&output);

        // begin + end per file + summary
        assert_eq!(messages.len(), 5);
        // File a has 2 matches
        assert_eq!(messages[1]["data"]["matched"], 2);
        // File b has 1 match
        assert_eq!(messages[3]["data"]["matched"], 1);
        assert_eq!(messages[4]["type"], "summary");
    }

    #[test]
    fn json_empty_results() {
        let result = SearchResult {
            definitions: vec![],
            read_errors: vec![],
            parse_failures: vec![],
        };
        let output = format_json_output(
            &result,
            OutputMode::Normal {
                survey: false,
                no_signature: false,
                no_filename: false,
                heading: false,
            },
        );
        let messages = parse_json_lines(&output);

        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0]["type"], "summary");
        assert_eq!(messages[0]["data"]["matched"], 0);
        assert_eq!(messages[0]["data"]["files"], 0);
    }

    #[test]
    fn json_summary_includes_errors() {
        let result = SearchResult {
            definitions: vec![make_fd(
                "src/a.rs",
                vec![make_def(DefKind::Function, "fn foo()", 1, 5, "foo")],
            )],
            read_errors: vec![FileError {
                path: PathBuf::from("src/missing.rs"),
                message: "not found".into(),
            }],
            parse_failures: vec![FileError {
                path: PathBuf::from("src/broken.rs"),
                message: "parse error".into(),
            }],
        };
        let output = format_json_output(
            &result,
            OutputMode::Normal {
                survey: false,
                no_signature: false,
                no_filename: false,
                heading: false,
            },
        );
        let messages = parse_json_lines(&output);

        let summary = messages.last().unwrap();
        assert_eq!(summary["type"], "summary");
        assert_eq!(summary["data"]["errors"], 2);
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

    #[test]
    fn json_each_line_is_valid_json() {
        let result = SearchResult {
            definitions: vec![make_fd(
                "src/a.rs",
                vec![
                    make_def(DefKind::Function, "fn foo()", 1, 5, "foo"),
                    make_def(DefKind::Class, "struct Bar { x: i32 }", 10, 20, "Bar"),
                ],
            )],
            read_errors: vec![],
            parse_failures: vec![],
        };
        let output = format_json_output(
            &result,
            OutputMode::Normal {
                survey: false,
                no_signature: false,
                no_filename: false,
                heading: false,
            },
        );
        for line in output.lines() {
            let _: serde_json::Value = serde_json::from_str(line)
                .unwrap_or_else(|e| panic!("invalid JSON: {line}\nerror: {e}"));
        }
    }

    #[test]
    fn json_long_signature_truncated() {
        let long_sig = "x".repeat(300);
        let result = SearchResult {
            definitions: vec![make_fd(
                "f.rs",
                vec![DefContent {
                    kind: DefKind::Function,
                    lines: [1, 1],
                    signature: long_sig,
                    scope: "foo".into(),
                }],
            )],
            read_errors: vec![],
            parse_failures: vec![],
        };
        let output = format_json_output(
            &result,
            OutputMode::Normal {
                survey: false,
                no_signature: false,
                no_filename: false,
                heading: false,
            },
        );
        let messages = parse_json_lines(&output);
        let match_msg = messages.iter().find(|m| m["type"] == "match").unwrap();
        let sig = match_msg["data"]["signature"].as_str().unwrap();
        assert!(sig.ends_with(" [truncated]"));
        assert!(sig.len() < 300);
        assert!(sig.len() <= MAX_SIGNATURE_LEN + 12); // +12 for " [truncated]"
    }

    // --- write_errors (stderr output) ---

    #[test]
    fn write_errors_outputs_per_file() {
        let result = SearchResult {
            definitions: vec![],
            read_errors: vec![FileError {
                path: PathBuf::from("src/missing.py"),
                message: "permission denied".into(),
            }],
            parse_failures: vec![FileError {
                path: PathBuf::from("src/broken.rs"),
                message: "tree-sitter parse failure".into(),
            }],
        };
        let mut buf = Vec::new();
        write_errors(&mut buf, &result, false);
        let output = String::from_utf8(buf).unwrap();
        let lines: Vec<&str> = output.lines().collect();
        assert_eq!(lines.len(), 2);
        assert!(lines[0].starts_with("peek: "));
        assert!(lines[0].contains("src/missing.py"));
        assert!(lines[0].contains("permission denied"));
        assert!(lines[1].starts_with("peek: "));
        assert!(lines[1].contains("src/broken.rs"));
        assert!(lines[1].contains("tree-sitter parse failure"));
    }

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

    // --- single-line range collapse ---

    #[test]
    fn single_line_collapses_with_filename_and_signature() {
        let result = SearchResult {
            definitions: vec![make_fd(
                "src/abc.py",
                vec![make_def(DefKind::Function, "def foo()", 15, 15, "foo")],
            )],
            read_errors: vec![],
            parse_failures: vec![],
        };
        let output = format_output(
            "foo",
            ".",
            &result,
            OutputMode::Normal {
                survey: false,
                no_signature: false,
                no_filename: false,
                heading: false,
            },
        );
        assert_eq!(output, "src/abc.py:15 [function/foo] def foo()");
    }

    #[test]
    fn single_line_collapses_with_filename_no_signature() {
        let result = SearchResult {
            definitions: vec![make_fd(
                "src/abc.py",
                vec![make_def(DefKind::Function, "def foo()", 15, 15, "foo")],
            )],
            read_errors: vec![],
            parse_failures: vec![],
        };
        let output = format_output(
            "foo",
            ".",
            &result,
            OutputMode::Normal {
                survey: false,
                no_signature: true,
                no_filename: false,
                heading: false,
            },
        );
        assert_eq!(output, "src/abc.py:15 [function/foo]");
    }

    #[test]
    fn single_line_collapses_no_filename_with_signature() {
        let result = SearchResult {
            definitions: vec![make_fd(
                "src/abc.py",
                vec![make_def(DefKind::Function, "def foo()", 15, 15, "foo")],
            )],
            read_errors: vec![],
            parse_failures: vec![],
        };
        let output = format_output(
            "foo",
            ".",
            &result,
            OutputMode::Normal {
                survey: false,
                no_signature: false,
                no_filename: true,
                heading: false,
            },
        );
        assert_eq!(output, "15 [function/foo] def foo()");
    }

    #[test]
    fn single_line_collapses_no_filename_no_signature() {
        let result = SearchResult {
            definitions: vec![make_fd(
                "src/abc.py",
                vec![make_def(DefKind::Function, "def foo()", 15, 15, "foo")],
            )],
            read_errors: vec![],
            parse_failures: vec![],
        };
        let output = format_output(
            "foo",
            ".",
            &result,
            OutputMode::Normal {
                survey: false,
                no_signature: true,
                no_filename: true,
                heading: false,
            },
        );
        assert_eq!(output, "15 [function/foo]");
    }

    // --- no_filename mode ---

    #[test]
    fn normal_mode_no_filename_with_signature() {
        let result = SearchResult {
            definitions: vec![make_fd(
                "src/abc.py",
                vec![make_def(
                    DefKind::Function,
                    "def process() -> bool",
                    45,
                    62,
                    "process",
                )],
            )],
            read_errors: vec![],
            parse_failures: vec![],
        };
        let output = format_output(
            "process",
            ".",
            &result,
            OutputMode::Normal {
                survey: false,
                no_signature: false,
                no_filename: true,
                heading: false,
            },
        );
        assert_eq!(output, "45-62 [function/process] def process() -> bool");
        assert!(!output.contains("src/abc.py"));
    }

    #[test]
    fn normal_mode_no_filename_without_signature() {
        let result = SearchResult {
            definitions: vec![make_fd(
                "src/abc.py",
                vec![make_def(
                    DefKind::Function,
                    "def process() -> bool",
                    45,
                    62,
                    "process",
                )],
            )],
            read_errors: vec![],
            parse_failures: vec![],
        };
        let output = format_output(
            "process",
            ".",
            &result,
            OutputMode::Normal {
                survey: false,
                no_signature: true,
                no_filename: true,
                heading: false,
            },
        );
        assert_eq!(output, "45-62 [function/process]");
    }

    #[test]
    fn normal_mode_no_filename_multiple_defs() {
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
                no_signature: false,
                no_filename: true,
                heading: false,
            },
        );
        let lines: Vec<&str> = output.lines().collect();
        assert_eq!(lines[0], "1-5 [class/Foo] class Foo");
        assert_eq!(lines[1], "10-20 [function/Foo.bar] def bar()");
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
    fn count_mode_no_filename_multiple_files() {
        let result = SearchResult {
            definitions: vec![
                make_fd(
                    "src/models.py",
                    vec![
                        make_def(DefKind::Class, "class Foo", 1, 5, "Foo"),
                        make_def(DefKind::Function, "def bar()", 10, 20, "Foo.bar"),
                    ],
                ),
                make_fd(
                    "src/handler.py",
                    vec![make_def(
                        DefKind::Function,
                        "def process()",
                        15,
                        30,
                        "process",
                    )],
                ),
            ],
            read_errors: vec![],
            parse_failures: vec![],
        };
        let output = format_output("foo", ".", &result, OutputMode::Count { no_filename: true });
        let lines: Vec<&str> = output.lines().collect();
        assert_eq!(lines[0], "2");
        assert_eq!(lines[1], "1");
    }

    // --- survey mode (contained definitions show abbreviated format) ---

    #[test]
    fn survey_contained_shows_abbreviated() {
        // enum 11-16 contains variants 13, 14, 15
        let result = SearchResult {
            definitions: vec![make_fd(
                "src/pattern.rs",
                vec![
                    make_def(
                        DefKind::Enum,
                        "#[derive(Debug, Clone, Copy, PartialEq, Eq)] pub enum CaseSensitivity",
                        11,
                        16,
                        "CaseSensitivity",
                    ),
                    make_def(
                        DefKind::Variant,
                        "Sensitive",
                        13,
                        13,
                        "CaseSensitivity.Sensitive",
                    ),
                    make_def(
                        DefKind::Variant,
                        "Insensitive",
                        14,
                        14,
                        "CaseSensitivity.Insensitive",
                    ),
                    make_def(
                        DefKind::Variant,
                        "SmartCase",
                        15,
                        15,
                        "CaseSensitivity.SmartCase",
                    ),
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
                no_signature: false,
                no_filename: false,
                heading: false,
            },
        );
        let lines: Vec<&str> = output.lines().collect();
        assert_eq!(lines.len(), 4);
        // Top-level: full format
        assert_eq!(
            lines[0],
            "src/pattern.rs:11-16 [enum/CaseSensitivity] #[derive(Debug, Clone, Copy, PartialEq, Eq)] pub enum CaseSensitivity"
        );
        // Contained: abbreviated (no file prefix, no [kind/scope])
        assert_eq!(lines[1], "13 Sensitive");
        assert_eq!(lines[2], "14 Insensitive");
        assert_eq!(lines[3], "15 SmartCase");
    }

    #[test]
    fn survey_mixed_contained_and_toplevel() {
        // enum 11-16 with variants, then enum 18-22 with variants, then function 24-29
        let result = SearchResult {
            definitions: vec![make_fd(
                "src/pattern.rs",
                vec![
                    make_def(
                        DefKind::Enum,
                        "pub enum CaseSensitivity",
                        11,
                        16,
                        "CaseSensitivity",
                    ),
                    make_def(
                        DefKind::Variant,
                        "Sensitive",
                        13,
                        13,
                        "CaseSensitivity.Sensitive",
                    ),
                    make_def(
                        DefKind::Variant,
                        "Insensitive",
                        14,
                        14,
                        "CaseSensitivity.Insensitive",
                    ),
                    make_def(
                        DefKind::Variant,
                        "SmartCase",
                        15,
                        15,
                        "CaseSensitivity.SmartCase",
                    ),
                    make_def(DefKind::Enum, "pub enum MatchMode", 18, 22, "MatchMode"),
                    make_def(DefKind::Variant, "Regex", 20, 20, "MatchMode.Regex"),
                    make_def(DefKind::Variant, "All", 21, 21, "MatchMode.All"),
                    make_def(
                        DefKind::Function,
                        "fn is_regex_meta(c: char) -> bool",
                        24,
                        29,
                        "is_regex_meta",
                    ),
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
                no_signature: false,
                no_filename: false,
                heading: false,
            },
        );
        let lines: Vec<&str> = output.lines().collect();
        assert_eq!(lines.len(), 8);
        // Top-level defs: full format
        assert_eq!(
            lines[0],
            "src/pattern.rs:11-16 [enum/CaseSensitivity] pub enum CaseSensitivity"
        );
        // Contained in 11-16
        assert_eq!(lines[1], "13 Sensitive");
        assert_eq!(lines[2], "14 Insensitive");
        assert_eq!(lines[3], "15 SmartCase");
        // Top-level: full format (18-22 not contained in 11-16)
        assert_eq!(
            lines[4],
            "src/pattern.rs:18-22 [enum/MatchMode] pub enum MatchMode"
        );
        // Contained in 18-22
        assert_eq!(lines[5], "20 Regex");
        assert_eq!(lines[6], "21 All");
        // Top-level: full format (24-29 not contained in anything)
        assert_eq!(
            lines[7],
            "src/pattern.rs:24-29 [function/is_regex_meta] fn is_regex_meta(c: char) -> bool"
        );
    }

    #[test]
    fn survey_contained_multiline_abbreviated() {
        // Multiline contained def shows start-end (not collapsed)
        let result = SearchResult {
            definitions: vec![make_fd(
                "src/lib.rs",
                vec![
                    make_def(DefKind::Class, "pub class Foo", 1, 50, "Foo"),
                    make_def(DefKind::Function, "def run(self)", 5, 30, "Foo.run"),
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
                no_signature: false,
                no_filename: false,
                heading: false,
            },
        );
        let lines: Vec<&str> = output.lines().collect();
        assert_eq!(lines[0], "src/lib.rs:1-50 [class/Foo] pub class Foo");
        // Contained multiline: shows start-end range, no file, no [kind/scope]
        assert_eq!(lines[1], "5-30 def run(self)");
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
    fn survey_no_filename_abbreviated_has_no_file_prefix() {
        let result = SearchResult {
            definitions: vec![make_fd(
                "src/lib.rs",
                vec![
                    make_def(DefKind::Enum, "pub enum Foo", 1, 10, "Foo"),
                    make_def(DefKind::Variant, "A", 3, 3, "Foo.A"),
                    make_def(DefKind::Function, "fn bar()", 15, 20, "bar"),
                ],
            )],
            read_errors: vec![],
            parse_failures: vec![],
        };
        let output = format_output(
            "*",
            ".",
            &result,
            OutputMode::Normal {
                survey: true,
                no_signature: false,
                no_filename: true,
                heading: false,
            },
        );
        let lines: Vec<&str> = output.lines().collect();
        // Full format: no file prefix (no_filename=true)
        assert_eq!(lines[0], "1-10 [enum/Foo] pub enum Foo");
        // Abbreviated: also no file prefix
        assert_eq!(lines[1], "3 A");
        // Full format
        assert_eq!(lines[2], "15-20 [function/bar] fn bar()");
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

    #[test]
    fn survey_non_survey_mode_unchanged() {
        // Verify that survey=false produces the same output as before
        let result = SearchResult {
            definitions: vec![make_fd(
                "src/lib.rs",
                vec![
                    make_def(DefKind::Enum, "pub enum Foo", 1, 10, "Foo"),
                    make_def(DefKind::Variant, "A", 3, 3, "Foo.A"),
                    make_def(DefKind::Function, "fn bar()", 15, 20, "bar"),
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
                no_signature: false,
                no_filename: false,
                heading: false,
            },
        );
        let lines: Vec<&str> = output.lines().collect();
        assert_eq!(lines.len(), 3);
        // Non-survey: all defs show full format
        assert_eq!(lines[0], "src/lib.rs:1-10 [enum/Foo] pub enum Foo");
        assert_eq!(lines[1], "src/lib.rs:3 [variant/Foo.A] A");
        assert_eq!(lines[2], "src/lib.rs:15-20 [function/bar] fn bar()");
    }

    // --- heading mode ---

    #[test]
    fn heading_mode_multiple_files() {
        let result = SearchResult {
            definitions: vec![
                make_fd(
                    "src/models.py",
                    vec![
                        make_def(DefKind::Class, "class MyClass(Base)", 42, 85, "MyClass"),
                        make_def(
                            DefKind::Function,
                            "def process()",
                            90,
                            100,
                            "MyClass.process",
                        ),
                    ],
                ),
                make_fd(
                    "src/handler.py",
                    vec![make_def(
                        DefKind::Function,
                        "def run(self)",
                        10,
                        20,
                        "Handler.run",
                    )],
                ),
            ],
            read_errors: vec![],
            parse_failures: vec![],
        };
        let output = format_output(
            "run",
            "src/",
            &result,
            OutputMode::Normal {
                survey: false,
                no_signature: false,
                no_filename: false,
                heading: true,
            },
        );
        let lines: Vec<&str> = output.lines().collect();
        // File 1: heading + 2 defs without file prefix
        assert_eq!(lines[0], "src/models.py");
        assert_eq!(lines[1], "42-85 [class/MyClass] class MyClass(Base)");
        assert_eq!(lines[2], "90-100 [function/MyClass.process] def process()");
        // Blank line separator between file groups
        assert_eq!(lines[3], "");
        // File 2: heading + 1 def without file prefix
        assert_eq!(lines[4], "src/handler.py");
        assert_eq!(lines[5], "10-20 [function/Handler.run] def run(self)");
        assert_eq!(lines.len(), 6);
    }

    #[test]
    fn heading_mode_with_no_filename_suppresses_heading() {
        let result = SearchResult {
            definitions: vec![make_fd(
                "src/models.py",
                vec![make_def(DefKind::Class, "class Foo", 1, 5, "Foo")],
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
                no_signature: false,
                no_filename: true,
                heading: true,
            },
        );
        // heading + no_filename: no heading line, no file prefix
        assert_eq!(output, "1-5 [class/Foo] class Foo");
        assert!(!output.contains("src/models.py"));
    }

    #[test]
    fn heading_mode_single_file_group() {
        let result = SearchResult {
            definitions: vec![make_fd(
                "src/lib.rs",
                vec![
                    make_def(DefKind::Function, "fn foo()", 1, 5, "foo"),
                    make_def(DefKind::Function, "fn bar()", 10, 20, "bar"),
                ],
            )],
            read_errors: vec![],
            parse_failures: vec![],
        };
        let output = format_output(
            "fn",
            "src/",
            &result,
            OutputMode::Normal {
                survey: false,
                no_signature: false,
                no_filename: false,
                heading: true,
            },
        );
        let lines: Vec<&str> = output.lines().collect();
        // Heading line + 2 defs without file prefix
        assert_eq!(lines[0], "src/lib.rs");
        assert_eq!(lines[1], "1-5 [function/foo] fn foo()");
        assert_eq!(lines[2], "10-20 [function/bar] fn bar()");
        assert_eq!(lines.len(), 3);
    }

    #[test]
    fn heading_mode_with_survey() {
        let result = SearchResult {
            definitions: vec![make_fd(
                "src/pattern.rs",
                vec![
                    make_def(DefKind::Enum, "pub enum Foo", 1, 10, "Foo"),
                    make_def(DefKind::Variant, "A", 3, 3, "Foo.A"),
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
                no_signature: false,
                no_filename: false,
                heading: true,
            },
        );
        let lines: Vec<&str> = output.lines().collect();
        // Heading line + full def + abbreviated + full def (no file prefix on any)
        assert_eq!(lines[0], "src/pattern.rs");
        assert_eq!(lines[1], "1-10 [enum/Foo] pub enum Foo");
        assert_eq!(lines[2], "3 A");
        assert_eq!(lines[3], "15-20 [function/bar] fn bar()");
        assert_eq!(lines.len(), 4);
    }

    #[test]
    fn heading_false_is_current_behavior() {
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
        let output = format_output(
            "foo",
            ".",
            &result,
            OutputMode::Normal {
                survey: false,
                no_signature: false,
                no_filename: false,
                heading: false,
            },
        );
        let lines: Vec<&str> = output.lines().collect();
        // heading=false: current behavior with file prefix on each line
        assert_eq!(lines[0], "src/a.rs:1-5 [function/foo] fn foo()");
        assert_eq!(lines[1], "src/b.rs:10-20 [class/Bar] struct Bar");
        assert_eq!(lines.len(), 2);
    }
}
