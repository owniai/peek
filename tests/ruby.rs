mod common;

use common::{parse_defs, peek};

// === Ruby scope verification & edge-case tests ===

#[test]
fn peek_for_ruby_singleton_method() {
    let output = peek(&["-k", "method", "find_by_email", "tests/fixtures/ruby"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "MyApp::Models::User::find_by_email" && r.kind == "method")
    );
}

#[test]
fn peek_for_ruby_nested_module_scope() {
    let output = peek(&["-k", "module", "Models", "tests/fixtures/ruby"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "MyApp::Models" && r.kind == "module")
    );
}

#[test]
fn peek_for_ruby_class_in_module_scope() {
    let output = peek(&["-k", "class", "User", "tests/fixtures/ruby"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "MyApp::Models::User" && r.kind == "class")
    );
}

#[test]
fn peek_for_ruby_method_scope() {
    let output = peek(&["-k", "method", "display_name", "tests/fixtures/ruby"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "MyApp::Models::User::display_name" && r.kind == "method")
    );
}

#[test]
fn peek_for_ruby_const_in_class_scope() {
    let output = peek(&["-k", "const", "DEFAULT_ROLE", "tests/fixtures/ruby"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "MyApp::Models::User::DEFAULT_ROLE" && r.kind == "const")
    );
}

#[test]
fn peek_for_ruby_nested_class_scope() {
    let output = peek(&["-k", "class", "Item", "tests/fixtures/ruby"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "Container::Item" && r.kind == "class")
    );
}

#[test]
fn peek_for_ruby_multiple_consts() {
    let output = peek(&["-k", "const", "SMTP_PORT", "tests/fixtures/ruby"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "MyApp::Services::EmailService::SMTP_PORT" && r.kind == "const")
    );

    let output2 = peek(&["-k", "const", "MAX_LOGIN_ATTEMPTS", "tests/fixtures/ruby"]);
    let stdout2 = String::from_utf8_lossy(&output2.stdout);
    assert!(output2.status.success());
    let results2 = parse_defs(&stdout2);
    assert!(
        results2
            .iter()
            .any(|r| r.scope == "MyApp::Models::User::MAX_LOGIN_ATTEMPTS" && r.kind == "const")
    );
}

#[test]
fn peek_for_ruby_no_false_positive() {
    // Comments and attr_reader should not be extracted
    let output = peek(&["attr_reader", "tests/fixtures/ruby"]);
    assert_eq!(
        output.status.code(),
        Some(1),
        "expected no match for attr_reader"
    );
}

// === Ruby top-level kind+scope verification tests ===

#[test]
fn peek_for_ruby_top_level_module() {
    let output = peek(&["-k", "module", "MyApp", "tests/fixtures/ruby"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "MyApp" && r.kind == "module")
    );
}

#[test]
fn peek_for_ruby_top_level_class() {
    let output = peek(&["-k", "class", "Container", "tests/fixtures/ruby"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "Container" && r.kind == "class")
    );
}

#[test]
fn peek_for_ruby_top_level_const() {
    let output = peek(&["-k", "const", "APP_VERSION", "tests/fixtures/ruby"]);
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
fn peek_for_ruby_top_level_function() {
    let output = peek(&["-k", "method", "global_helper", "tests/fixtures/ruby"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "global_helper" && r.kind == "method")
    );
}

#[test]
fn peek_for_ruby_class_inheritance_signature() {
    let output = peek(&["-k", "class", "ApplicationError", "tests/fixtures/ruby"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    let def = results
        .iter()
        .find(|r| r.scope == "ApplicationError" && r.kind == "class")
        .expect("expected ApplicationError class definition");
    assert!(
        def.signature.contains("< StandardError"),
        "expected signature to contain '< StandardError', got: {}",
        def.signature
    );
}
