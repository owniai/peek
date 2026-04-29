mod common;

use common::{parse_defs, peek};

// === Bash scope verification & edge-case tests ===

#[test]
fn peek_for_bash_nested_function_scope() {
    let output = peek(&["-k", "function", "load_config", "tests/fixtures/bash"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "configure::load_config" && r.kind == "function")
    );
}

#[test]
fn peek_for_bash_const_in_function_scope() {
    let output = peek(&["-k", "const", "CONFIG_DIR", "tests/fixtures/bash"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "configure::CONFIG_DIR" && r.kind == "const")
    );
}

#[test]
fn peek_for_bash_posix_nested_function_scope() {
    let output = peek(&["-k", "function", "validate_env", "tests/fixtures/bash"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "deploy::validate_env" && r.kind == "function")
    );
}

#[test]
fn peek_for_bash_posix_nested_const_scope() {
    let output = peek(&["-k", "const", "DEPLOY_TARGET", "tests/fixtures/bash"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "deploy::DEPLOY_TARGET" && r.kind == "const")
    );
}

#[test]
fn peek_for_bash_deeply_nested_scope() {
    let output = peek(&["-k", "function", "finalize", "tests/fixtures/bash"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "main::init::finalize" && r.kind == "function")
    );
}

#[test]
fn peek_for_bash_deeply_nested_const() {
    let output = peek(&["-k", "const", "INIT_FLAG", "tests/fixtures/bash"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "main::init::INIT_FLAG" && r.kind == "const")
    );
}

#[test]
fn peek_for_bash_multiple_readonly() {
    let output = peek(&["-k", "const", "SMTP_PORT", "tests/fixtures/bash"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    assert!(stdout.contains("SMTP_PORT"));

    let output2 = peek(&["-k", "const", "MAX_CONNECTIONS", "tests/fixtures/bash"]);
    let stdout2 = String::from_utf8_lossy(&output2.stdout);
    assert!(output2.status.success());
    assert!(stdout2.contains("MAX_CONNECTIONS"));
}

#[test]
fn peek_for_bash_declare_r_in_function_scope() {
    let output = peek(&["-k", "const", "SETUP_PATH", "tests/fixtures/bash"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "setup::SETUP_PATH" && r.kind == "const")
    );
}

#[test]
fn peek_for_bash_no_false_positive_local() {
    // local variables should never be extracted
    let output = peek(&["temp_var", "tests/fixtures/bash"]);
    assert_eq!(
        output.status.code(),
        Some(1),
        "expected no match for local variable"
    );
}

#[test]
fn peek_for_bash_no_false_positive_declare() {
    // declare without -r should never be extracted
    let output = peek(&["NORMAL_VAR", "tests/fixtures/bash"]);
    assert_eq!(
        output.status.code(),
        Some(1),
        "expected no match for declare without -r"
    );
}

// === Bash top-level kind+scope verification tests ===

#[test]
fn peek_for_bash_function_keyword() {
    let output = peek(&["-k", "function", "build_project", "tests/fixtures/bash"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "build_project" && r.kind == "function")
    );
}

#[test]
fn peek_for_bash_posix_function() {
    let output = peek(&["-k", "function", "run_tests", "tests/fixtures/bash"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "run_tests" && r.kind == "function")
    );
}

#[test]
fn peek_for_bash_readonly_const() {
    let output = peek(&["-k", "const", "APP_VERSION", "tests/fixtures/bash"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "APP_VERSION" && r.kind == "const")
    );
}

#[test]
fn peek_for_bash_declare_r_const() {
    let output = peek(&["-k", "const", "MAX_RETRIES", "tests/fixtures/bash"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "MAX_RETRIES" && r.kind == "const")
    );
}
