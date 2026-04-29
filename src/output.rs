use crate::pipeline::SearchResult;
use serde::Serialize;
use std::borrow::Cow;
use std::fmt::Write;
use std::path::Path;

/// Maximum signature length for display. Truncated with "..." suffix if exceeded.
const MAX_SIGNATURE_LEN: usize = 256;

#[derive(Copy, Clone)]
pub enum OutputMode {
    Normal { no_signature: bool },
    FilesOnly,
    Count,
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
            OutputMode::Count => {
                for fd in &result.definitions {
                    let _ = writeln!(out, "{}:{}", relativize_path(&fd.file, &cwd), fd.defs.len());
                }
            }
            OutputMode::Normal { no_signature } => {
                for fd in &result.definitions {
                    let file = relativize_path(&fd.file, &cwd);
                    for def in &fd.defs {
                        write_def(&mut out, &file, def, no_signature);
                        out.push('\n');
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
                    out.push_str(&serialize(&JsonMessage::Match {
                        path: path.clone(),
                        line_start: def.lines[0],
                        line_end: def.lines[1],
                        kind: def.kind.display_tag().to_string(),
                        scope: def.scope.clone(),
                        signature: def.signature.clone(),
                    }));
                    out.push('\n');
                }
            }
            OutputMode::FilesOnly | OutputMode::Count => {}
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
fn relativize_path<'a>(path: &'a Path, base: &Path) -> Cow<'a, str> {
    match path.strip_prefix(base) {
        Ok(relative) => relative.to_string_lossy(),
        Err(_) => path.to_string_lossy(),
    }
}

/// Format: `file:start-end [kind/scope] signature` (or without signature if `no_signature`).
fn write_def(out: &mut String, file: &str, def: &crate::model::DefContent, no_signature: bool) {
    let kind = def.kind.display_tag();
    if no_signature {
        write!(
            out,
            "{}:{}-{} [{}/{}]",
            file, def.lines[0], def.lines[1], kind, def.scope
        )
        .unwrap();
    } else {
        let sig = truncate_str(&def.signature, MAX_SIGNATURE_LEN);
        let truncation = if def.signature.len() > MAX_SIGNATURE_LEN {
            "..."
        } else {
            ""
        };
        write!(
            out,
            "{}:{}-{} [{}/{}] {}{}",
            file, def.lines[0], def.lines[1], kind, def.scope, sig, truncation
        )
        .unwrap();
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
                no_signature: false,
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
                no_signature: false,
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
                no_signature: false,
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
                no_signature: false,
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
                no_signature: false,
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
                no_signature: false,
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
                no_signature: false,
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
                no_signature: false,
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
            OutputMode::Normal { no_signature: true },
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
            OutputMode::Normal { no_signature: true },
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
        let output = format_output("foo", ".", &result, OutputMode::Count);
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
        let output = format_output("X", ".", &result, OutputMode::Count);
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
                no_signature: false,
            },
        );
        assert!(output.contains("fn foo()"));
        assert!(!output.contains("..."));
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
                no_signature: false,
            },
        );
        assert!(output.ends_with("..."));
        // Output should be shorter than the original 300-char signature
        let sig_part = output.split(']').next_back().unwrap().trim();
        assert!(sig_part.len() < 300);
        assert!(sig_part.len() <= MAX_SIGNATURE_LEN + 3); // +3 for "..."
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
                no_signature: false,
            },
        );
        assert!(!output.contains("..."));
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
                no_signature: false,
            },
        );
        assert!(output.ends_with("..."));
    }

    // --- relativize_path ---

    #[test]
    fn relativize_strips_cwd_prefix_from_absolute_path() {
        let cwd = std::env::current_dir().unwrap();
        let abs_path = cwd.join("src").join("main.rs");
        let result = relativize_path(&abs_path, &cwd);
        assert_eq!(
            result,
            PathBuf::from_iter(["src", "main.rs"]).to_string_lossy()
        );
    }

    #[test]
    fn relativize_preserves_non_cwd_absolute_path() {
        let cwd = std::env::current_dir().unwrap();
        // Path outside CWD — strip_prefix fails, original path kept
        let outside = cwd
            .parent()
            .unwrap_or(Path::new("/"))
            .join("other_project")
            .join("file.rs");
        let result = relativize_path(&outside, &cwd);
        assert_eq!(result, outside.to_string_lossy());
    }

    #[test]
    fn relativize_preserves_relative_path() {
        let cwd = std::env::current_dir().unwrap();
        let rel = PathBuf::from("src/main.rs");
        let result = relativize_path(&rel, &cwd);
        assert_eq!(result, rel.to_string_lossy());
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
                no_signature: false,
            },
        );
        let expected_path = PathBuf::from_iter(["src", "models.py"]);
        let expected_file = expected_path.to_string_lossy();
        assert!(
            output.starts_with(&*expected_file),
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
        let expected_path = PathBuf::from_iter(["src", "models.py"]);
        assert_eq!(output, expected_path.to_string_lossy());
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
        let output = format_output("Foo", ".", &result, OutputMode::Count);
        let expected_path = PathBuf::from_iter(["src", "models.py"]);
        assert_eq!(output, format!("{}:2", expected_path.to_string_lossy()));
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
                no_signature: false,
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
                no_signature: false,
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
        let output = format_json_output(&result, OutputMode::Count);
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
                no_signature: false,
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
                no_signature: false,
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
        let output = format_json_output(&result, OutputMode::Normal { no_signature: true });
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
                no_signature: false,
            },
        );
        for line in output.lines() {
            let _: serde_json::Value = serde_json::from_str(line)
                .unwrap_or_else(|e| panic!("invalid JSON: {line}\nerror: {e}"));
        }
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
}
