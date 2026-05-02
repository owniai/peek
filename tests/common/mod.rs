use regex::Regex;
use std::process::Command;
use std::sync::LazyLock;

#[derive(Debug)]
#[allow(dead_code)]
pub struct DefLine {
    pub kind: String,
    pub scope: String,
    pub signature: String,
    pub start: usize,
    pub end: usize,
}

static DEF_LINE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?:(?:[A-Za-z]:[^:]+|[^:]+):)?(\d+)-(\d+) \[(\w+)/([^\]]+)\](?: (.*))?").unwrap()
});

/// Run peek binary with given arguments.
#[allow(dead_code)]
pub fn peek(args: &[&str]) -> std::process::Output {
    let bin = env!("CARGO_BIN_EXE_peek");
    Command::new(bin)
        .args(args)
        .output()
        .expect("Failed to run peek")
}

/// Get peek stdout as String.
#[allow(dead_code)]
pub fn peek_stdout(args: &[&str]) -> String {
    String::from_utf8_lossy(&peek(args).stdout).to_string()
}

/// Parse definition lines from peek output.
/// Handles both formats: `file:start-end [kind/scope] signature` and `start-end [kind/scope] signature`
#[allow(dead_code)]
pub fn parse_defs(stdout: &str) -> Vec<DefLine> {
    stdout
        .lines()
        .filter_map(|line| {
            let caps = DEF_LINE_RE.captures(line)?;
            Some(DefLine {
                kind: caps[3].to_string(),
                scope: caps[4].to_string(),
                signature: caps
                    .get(5)
                    .map(|m| m.as_str().to_string())
                    .unwrap_or_default(),
                start: caps[1].parse().ok()?,
                end: caps[2].parse().ok()?,
            })
        })
        .collect()
}

/// Assert that a peek search returns exactly one result with the expected scope and kind.
#[allow(dead_code)]
pub fn assert_scope(args: &[&str], expected_scope: &str, expected_kind: &str) {
    let output = peek(args);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "peek {:?} failed\nstderr: {}",
        args,
        String::from_utf8_lossy(&output.stderr)
    );
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == expected_scope && r.kind == expected_kind),
        "expected scope={expected_scope} kind={expected_kind}, got: {results:?}"
    );
}

/// Assert that a peek search returns no definitions (exit 1, silent stdout).
#[allow(dead_code)]
pub fn assert_no_defs(args: &[&str]) {
    let output = peek(args);
    assert_eq!(
        output.status.code(),
        Some(1),
        "expected exit 1 (no match), got {:?}\nstdout: {}\nstderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
