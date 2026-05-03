mod common;

use common::peek;

#[test]
fn peek_no_results() {
    let output = peek(&["DoesNotExist", "tests/fixtures"]);
    assert_eq!(output.status.code(), Some(1));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.is_empty(),
        "expected silent stdout on no match, got: {stdout}"
    );
}

#[test]
fn peek_invalid_path_exits_2() {
    let output = peek(&["foo", "/nonexistent/path"]);
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("do not exist") || stderr.contains("does not exist"));
}

#[test]
fn peek_path_lists_all_definitions() {
    let output = peek(&["tests/fixtures/python/basic_functions.py"]);
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("[function/"));
}

#[test]
fn peek_path_with_kind_filter() {
    let output = peek(&["-k", "function", "tests/fixtures/python/basic_functions.py"]);
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("[function/"));
    assert!(!stdout.contains("[class/"));
}

#[test]
fn peek_regexp_treats_positional_as_path() {
    // When -e is provided, positional args become paths (ripgrep-aligned)
    let output = peek(&[
        "-e",
        "simple_func",
        "tests/fixtures/python/basic_functions.py",
    ]);
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("[function/simple_func]"));
}
