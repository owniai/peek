mod common;

use common::{peek, peek_in};
use std::io::Write as IoWrite;

#[test]
fn cli_exit_code_0_on_match() {
    let output = peek(&["def", "top_level_func", "tests/fixtures/python/"]);
    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(!stdout.is_empty(), "match should produce output");
}

#[test]
fn cli_exit_code_1_on_no_match() {
    let output = peek(&["def", "DoesNotExist", "tests/fixtures"]);
    assert_eq!(output.status.code(), Some(1));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.is_empty(), "no match should produce empty stdout");
}

#[test]
fn cli_exit_code_2_on_error() {
    let output = peek(&["def", "foo", "/nonexistent/path"]);
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("do not exist") || stderr.contains("does not exist"));
}

#[test]
fn peek_absolute_file_path_search_succeeds() {
    // Bug #6 regression: searching an absolute file path must succeed
    let tmp = tempfile::tempdir().unwrap();
    let py_file = tmp.path().join("target_file.py");
    let mut f = std::fs::File::create(&py_file).unwrap();
    f.write_all(b"def my_abs_func(): pass\n").unwrap();
    f.flush().unwrap();

    let abs_path = py_file.to_string_lossy().to_string();
    let output = peek(&["def", "my_abs_func", &abs_path]);
    assert!(
        output.status.success(),
        "searching absolute path should succeed"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("my_abs_func"),
        "should find the function in absolute path file: {stdout}"
    );
}

#[test]
fn cli_no_subcommand_errors() {
    let output = peek(&["my_func"]);
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("def") || stderr.contains("subcommand"),
        "should suggest subcommands: {stderr}"
    );
}

#[test]
fn cli_outline_lists_definitions() {
    let output = peek(&["outline", "tests/fixtures/python/"]);
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(!stdout.is_empty(), "outline should produce output");
}

// --- register/unregister integration ---

#[test]
fn register_no_target_no_list_targets_errors() {
    let output = peek(&["register"]);
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("--target") || stderr.contains("--list-targets"),
        "should mention --target or --list-targets: {stderr}"
    );
}

#[test]
fn register_unknown_target_errors() {
    let output = peek(&["register", "--target", "nonexistent"]);
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("unknown target"),
        "should say unknown target: {stderr}"
    );
}

#[test]
fn register_claude_local_writes_mcp_json() {
    let dir = tempfile::tempdir().unwrap();
    let config_path = dir.path().join(".mcp.json");

    let output = peek_in(dir.path(), &["register", "--target", "claude", "--local"]);
    assert!(
        output.status.success(),
        "register should succeed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("created"),
        "should report created: {stdout}"
    );

    assert!(config_path.exists(), ".mcp.json should be created");
    let content = std::fs::read_to_string(&config_path).unwrap();
    let config: serde_json::Value = serde_json::from_str(&content).unwrap();
    assert_eq!(config["mcpServers"]["peek"]["command"], "peek");
    assert_eq!(
        config["mcpServers"]["peek"]["args"],
        serde_json::json!(["mcp"])
    );
}

#[test]
fn unregister_claude_local_removes_entry() {
    let dir = tempfile::tempdir().unwrap();
    let config_path = dir.path().join(".mcp.json");

    // Register first
    let output = peek_in(dir.path(), &["register", "--target", "claude", "--local"]);
    assert!(output.status.success());

    // Then unregister
    let output = peek_in(dir.path(), &["unregister", "--target", "claude", "--local"]);
    assert!(
        output.status.success(),
        "unregister should succeed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("updated"),
        "should report updated: {stdout}"
    );

    let content = std::fs::read_to_string(&config_path).unwrap();
    let config: serde_json::Value = serde_json::from_str(&content).unwrap();
    assert!(
        config.get("mcpServers").is_none(),
        "mcpServers should be removed"
    );
}

#[test]
fn register_claude_local_idempotent() {
    let dir = tempfile::tempdir().unwrap();

    let output1 = peek_in(dir.path(), &["register", "--target", "claude", "--local"]);
    assert!(output1.status.success());
    let stdout1 = String::from_utf8_lossy(&output1.stdout);
    assert!(stdout1.contains("created"));

    let output2 = peek_in(dir.path(), &["register", "--target", "claude", "--local"]);
    assert!(output2.status.success());
    let stdout2 = String::from_utf8_lossy(&output2.stdout);
    assert!(
        stdout2.contains("unchanged"),
        "second register should be unchanged: {stdout2}"
    );
}

#[test]
fn register_cursor_local_writes_mcp_json() {
    let dir = tempfile::tempdir().unwrap();
    let config_path = dir.path().join(".cursor").join("mcp.json");

    let output = peek_in(dir.path(), &["register", "--target", "cursor", "--local"]);
    assert!(
        output.status.success(),
        "cursor register should succeed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("created"),
        "should report created: {stdout}"
    );

    assert!(config_path.exists(), ".cursor/mcp.json should be created");
    let content = std::fs::read_to_string(&config_path).unwrap();
    let config: serde_json::Value = serde_json::from_str(&content).unwrap();
    assert_eq!(config["mcpServers"]["peek"]["command"], "peek");
    let args = config["mcpServers"]["peek"]["args"].as_array().unwrap();
    assert_eq!(args, &["mcp"]);
}

#[test]
fn unregister_cursor_local_removes_entry() {
    let dir = tempfile::tempdir().unwrap();
    let config_path = dir.path().join(".cursor").join("mcp.json");

    // Register then unregister
    peek_in(dir.path(), &["register", "--target", "cursor", "--local"]);
    let output = peek_in(dir.path(), &["unregister", "--target", "cursor", "--local"]);
    assert!(
        output.status.success(),
        "cursor unregister should succeed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let content = std::fs::read_to_string(&config_path).unwrap();
    let config: serde_json::Value = serde_json::from_str(&content).unwrap();
    assert!(config.get("mcpServers").is_none());
}

#[test]
fn register_codex_local_errors_not_supported() {
    let dir = tempfile::tempdir().unwrap();
    let output = peek_in(dir.path(), &["register", "--target", "codex", "--local"]);
    assert!(!output.status.success(), "codex local should fail");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("not support local"),
        "should say not supported: {stderr}"
    );
}

#[test]
fn register_cursor_and_codex_in_list_targets() {
    let output = peek(&["register", "--list-targets"]);
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("cursor"));
    assert!(stdout.contains("mcp.json") && stdout.contains("cursor"));
    assert!(stdout.contains("codex"));
    assert!(stdout.contains("config.toml"));
    assert!(stdout.contains("claude"));
}
