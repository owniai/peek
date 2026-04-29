mod common;

use common::{parse_defs, peek};

// === Dart scope verification & edge-case tests ===

#[test]
fn peek_for_dart_class_method_scope() {
    let output = peek(&["-k", "function", "greet", "tests/fixtures/dart"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "UserService.greet" && r.kind == "function")
    );
}

#[test]
fn peek_for_dart_getter_scope() {
    let output = peek(&["-k", "function", "displayName", "tests/fixtures/dart"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "UserService.displayName" && r.kind == "function")
    );
}

#[test]
fn peek_for_dart_mixin_method_scope() {
    let output = peek(&["-k", "function", "log", "tests/fixtures/dart"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "Loggable.log" && r.kind == "function")
    );
}

#[test]
fn peek_for_dart_extension_method_scope() {
    let output = peek(&["-k", "function", "repeated", "tests/fixtures/dart"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "StringExt.repeated" && r.kind == "function")
    );
}

#[test]
fn peek_for_dart_static_const_scope() {
    let output = peek(&["-k", "const", "DEFAULT_TIMEOUT", "tests/fixtures/dart"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "UserService.DEFAULT_TIMEOUT" && r.kind == "const")
    );
}

#[test]
fn peek_for_dart_multi_const() {
    // Multi-constant declarations extract all names via AST parsing.
    // All names in comma-separated const declarations are findable.

    // Clean cache for deterministic first-search behavior
    let _ = std::fs::remove_dir_all(".peek-cache");

    let output = peek(&["-k", "const", "CACHE_TTL", "tests/fixtures/dart"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "CACHE_TTL" && r.kind == "const")
    );
}

#[test]
fn peek_for_dart_annotation_class() {
    let output = peek(&["-k", "class", "UserService", "tests/fixtures/dart"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    assert!(stdout.contains("@deprecated"));
    assert!(stdout.contains("class UserService"));
}

#[test]
fn peek_for_dart_no_false_positive() {
    // Comments, import, library directives should never be extracted
    let output = peek(&["print", "tests/fixtures/dart"]);
    assert_eq!(
        output.status.code(),
        Some(1),
        "expected no match for non-definition 'print'"
    );
}

// === Dart top-level kind+scope verification tests ===

#[test]
fn peek_for_dart_top_level_class() {
    let output = peek(&["-k", "class", "UserService", "tests/fixtures/dart"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "UserService" && r.kind == "class")
    );
}

#[test]
fn peek_for_dart_abstract_class() {
    let output = peek(&["-k", "class", "BaseProcessor", "tests/fixtures/dart"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "BaseProcessor" && r.kind == "class")
    );
    let def = results.iter().find(|r| r.scope == "BaseProcessor").unwrap();
    assert!(
        def.signature.contains("abstract"),
        "expected signature to contain 'abstract', got: {}",
        def.signature
    );
}

#[test]
fn peek_for_dart_enum() {
    let output = peek(&["-k", "enum", "Status", "tests/fixtures/dart"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "Status" && r.kind == "enum")
    );
}

#[test]
fn peek_for_dart_mixin() {
    let output = peek(&["-k", "mixin", "Loggable", "tests/fixtures/dart"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "Loggable" && r.kind == "mixin")
    );
}

#[test]
fn peek_for_dart_extension() {
    let output = peek(&["-k", "extension", "StringExt", "tests/fixtures/dart"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "StringExt" && r.kind == "extension")
    );
}

#[test]
fn peek_for_dart_typedef() {
    let output = peek(&["-k", "type", "Callback", "tests/fixtures/dart"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "Callback" && r.kind == "type")
    );
}

#[test]
fn peek_for_dart_top_level_function() {
    let output = peek(&["-k", "function", "globalHelper", "tests/fixtures/dart"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "globalHelper" && r.kind == "function")
    );
}

#[test]
fn peek_for_dart_top_level_const() {
    let output = peek(&["-k", "const", "APP_VERSION", "tests/fixtures/dart"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "APP_VERSION" && r.kind == "const")
    );
}

// === Function-body definitions should NOT be extracted ===

#[test]
fn peek_for_dart_function_body_const_not_extracted() {
    let output = peek(&["-k", "const", "localConst", "tests/fixtures/dart"]);
    assert_eq!(
        output.status.code(),
        Some(1),
        "Expected no results for function-body const"
    );
}

#[test]
fn peek_for_dart_function_body_func_not_extracted() {
    let output = peek(&["-k", "function", "localHelper", "tests/fixtures/dart"]);
    assert_eq!(
        output.status.code(),
        Some(1),
        "Expected no results for function-body function"
    );
}
