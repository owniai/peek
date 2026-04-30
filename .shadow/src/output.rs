use crate::pipeline::SearchResult;
use serde::Serialize;
use std::borrow::Cow;
use std::fmt::Write;
use std::path::Path;

/// Maximum signature length for display. Truncated with "..." suffix if exceeded.
const MAX_SIGNATURE_LEN: usize = 256;

#[derive(Copy, Clone)]
pub enum OutputMode {
    Normal { no_signature: bool, no_filename: bool },
    FilesOnly,
    Count { no_filename: bool },
}

pub fn format_output(_name: &str, _path: &str, result: &SearchResult, mode: OutputMode) -> String // L17-54

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

pub fn format_json_output(result: &SearchResult, mode: OutputMode) -> String // L83-130

/// Convert an absolute path to a relative path by stripping the base directory prefix.
/// Falls back to the original path if stripping fails (e.g., different drive on Windows).
fn relativize_path<'a>(path: &'a Path, base: &Path) -> Cow<'a, str> // L134-139

/// Format: `file:start-end [kind/scope] signature` (or without signature/filename).
fn write_def(out: &mut String, file: &str, def: &crate::model::DefContent, no_signature: bool, no_filename: bool) // L142-183

fn truncate_str(s: &str, max_len: usize) -> &str // L185-195

/// Write non-fatal errors to the given writer (typically stderr).
/// Each error is printed as `peek: {path}: {message}` on a separate line.
/// When `suppress` is true, no output is produced (but exit code is still affected).
pub fn write_errors<W: std::io::Write>(wtr: &mut W, result: &SearchResult, suppress: bool) // L200-213

// #[cfg(test)] mod tests { ... } // L215-1291 (test module)
