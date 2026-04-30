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

static DEF_LINE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?:(?:[^:]+):)?(\d+)-(\d+) \[(\w+)/([^\]]+)\](?: (.*))?").unwrap());

/// Run peek binary with given arguments.
#[allow(dead_code)]
pub fn peek(args: &[&str]) -> std::process::Output // L20-26

/// Get peek stdout as String.
#[allow(dead_code)]
pub fn peek_stdout(args: &[&str]) -> String // L29-32

/// Parse definition lines from peek output.
/// Handles both formats: `file:start-end [kind/scope] signature` and `start-end [kind/scope] signature`
#[allow(dead_code)]
pub fn parse_defs(stdout: &str) -> Vec<DefLine> // L37-54

/// Assert that a peek search returns exactly one result with the expected scope and kind.
#[allow(dead_code)]
pub fn assert_scope(args: &[&str], expected_scope: &str, expected_kind: &str) // L58-74

/// Assert that a peek search returns no definitions (exit 1, silent stdout).
#[allow(dead_code)]
pub fn assert_no_defs(args: &[&str]) // L78-88
