#![allow(dead_code)]

use serde::Deserialize;
use std::collections::BTreeMap;
use std::process::Command;

/// Run peek binary with given arguments.
pub fn peek(args: &[&str]) -> std::process::Output {
    let bin = env!("CARGO_BIN_EXE_peek");
    Command::new(bin)
        .args(args)
        .output()
        .expect("Failed to run peek")
}

/// Run peek binary with given arguments and working directory.
pub fn peek_in(dir: &std::path::Path, args: &[&str]) -> std::process::Output {
    let bin = env!("CARGO_BIN_EXE_peek");
    Command::new(bin)
        .args(args)
        .current_dir(dir)
        .output()
        .expect("Failed to run peek")
}

// ── Expected snapshot infrastructure ──

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct ExpectedEntry {
    pub path: String,
    pub kind: String,
    pub scope: String,
    pub signature: String,
}

#[derive(Debug, Deserialize)]
struct JsonMessage {
    #[serde(rename = "type")]
    msg_type: String,
    data: serde_json::Value,
}

/// Parse `type=match` entries from peek's NDJSON output.
pub fn parse_json_matches(stdout: &str) -> Vec<ExpectedEntry> {
    stdout
        .lines()
        .filter_map(|line| {
            let msg: JsonMessage = serde_json::from_str(line).ok()?;
            if msg.msg_type != "match" {
                return None;
            }
            Some(ExpectedEntry {
                path: msg.data["path"].as_str()?.to_string(),
                kind: msg.data["kind"].as_str()?.to_string(),
                scope: msg.data["scope"].as_str()?.to_string(),
                signature: msg.data["signature"].as_str()?.to_string(),
            })
        })
        .collect()
}

/// Load expected entries from a JSONL file.
pub fn load_expected(path: &str) -> Vec<ExpectedEntry> {
    let content = std::fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("Failed to read expected file {path}: {e}"));
    content
        .lines()
        .filter_map(|line| {
            if line.is_empty() {
                return None;
            }
            serde_json::from_str::<ExpectedEntry>(line)
                .ok()
                .or_else(|| {
                    eprintln!("WARNING: failed to parse expected line: {line}");
                    None
                })
        })
        .collect()
}

#[derive(Debug)]
pub struct DiffReport {
    pub kind_mismatch: Vec<(String, String, String, String)>,
    pub missing: Vec<ExpectedEntry>,
    pub extra: Vec<ExpectedEntry>,
    pub sig_mismatch: Vec<(String, String, String)>,
}

impl DiffReport {
    pub fn has_hard_failures(&self) -> bool {
        !self.kind_mismatch.is_empty()
            || !self.missing.is_empty()
            || !self.extra.is_empty()
            || !self.sig_mismatch.is_empty()
    }
}

/// Compare expected entries against actual entries using scope as primary key.
/// Entries with the same scope are sorted by (kind, path) and matched pairwise.
pub fn compare_expected(expected: Vec<ExpectedEntry>, actual: Vec<ExpectedEntry>) -> DiffReport {
    let mut expected_by_scope: BTreeMap<String, Vec<ExpectedEntry>> = BTreeMap::new();
    for entry in expected {
        expected_by_scope
            .entry(entry.scope.clone())
            .or_default()
            .push(entry);
    }

    let mut actual_by_scope: BTreeMap<String, Vec<ExpectedEntry>> = BTreeMap::new();
    for entry in actual {
        actual_by_scope
            .entry(entry.scope.clone())
            .or_default()
            .push(entry);
    }

    fn sort_entries(entries: &mut [ExpectedEntry]) {
        entries.sort_by(|a, b| a.kind.cmp(&b.kind).then_with(|| a.path.cmp(&b.path)));
    }
    for entries in expected_by_scope.values_mut() {
        sort_entries(entries);
    }
    for entries in actual_by_scope.values_mut() {
        sort_entries(entries);
    }

    let mut report = DiffReport {
        kind_mismatch: Vec::new(),
        missing: Vec::new(),
        extra: Vec::new(),
        sig_mismatch: Vec::new(),
    };

    for (scope, expected_entries) in &expected_by_scope {
        let actual_entries = actual_by_scope
            .get(scope)
            .map(|v| v.as_slice())
            .unwrap_or_default();

        for (i, expected_e) in expected_entries.iter().enumerate() {
            if let Some(actual_e) = actual_entries.get(i) {
                if expected_e.kind != actual_e.kind {
                    report.kind_mismatch.push((
                        scope.clone(),
                        expected_e.kind.clone(),
                        actual_e.kind.clone(),
                        expected_e.path.clone(),
                    ));
                }
                if expected_e.signature != actual_e.signature {
                    report.sig_mismatch.push((
                        scope.clone(),
                        expected_e.signature.clone(),
                        actual_e.signature.clone(),
                    ));
                }
            } else {
                report.missing.push(expected_e.clone());
            }
        }

        if let Some(extra) = actual_entries.get(expected_entries.len()..) {
            for actual_e in extra {
                report.extra.push(actual_e.clone());
            }
        }
    }

    for (scope, actual_entries) in &actual_by_scope {
        if !expected_by_scope.contains_key(scope) {
            for actual_e in actual_entries {
                report.extra.push(actual_e.clone());
            }
        }
    }

    report
}

/// Format a DiffReport as a human-readable string.
pub fn format_diff(lang: &str, report: &DiffReport) -> String {
    if !report.has_hard_failures() {
        return format!("PASS: {lang}");
    }

    let total = report.kind_mismatch.len()
        + report.missing.len()
        + report.extra.len()
        + report.sig_mismatch.len();

    let mut out = format!("FAIL: {lang} — {total} differences\n");

    if !report.kind_mismatch.is_empty() {
        out.push_str(&format!(
            "\n  KIND_MISMATCH ({}):\n",
            report.kind_mismatch.len()
        ));
        for (scope, expected, actual, path) in &report.kind_mismatch {
            out.push_str(&format!(
                "    scope={scope} path={path} expected={expected} actual={actual}\n"
            ));
        }
    }

    if !report.missing.is_empty() {
        out.push_str(&format!("\n  MISSING ({}):\n", report.missing.len()));
        for e in &report.missing {
            out.push_str(&format!(
                "    scope={} kind={} path={}\n",
                e.scope, e.kind, e.path
            ));
        }
    }

    if !report.extra.is_empty() {
        out.push_str(&format!("\n  EXTRA ({}):\n", report.extra.len()));
        for e in &report.extra {
            out.push_str(&format!(
                "    scope={} kind={} path={}\n",
                e.scope, e.kind, e.path
            ));
        }
    }

    if !report.sig_mismatch.is_empty() {
        out.push_str(&format!(
            "\n  SIG_MISMATCH ({}):\n",
            report.sig_mismatch.len()
        ));
        for (scope, expected, actual) in &report.sig_mismatch {
            out.push_str(&format!(
                "    scope={scope} expected={expected} actual={actual}\n"
            ));
        }
    }

    out
}
