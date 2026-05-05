mod common;

use common::{parse_defs, peek};

// === Dart scope verification & edge-case tests ===

#[test]
fn peek_for_dart_class_method_scope() {
    let output = peek(&["-k", "method", "greet", "tests/fixtures/dart"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "UserService.greet" && r.kind == "method")
    );
}

#[test]
fn peek_for_dart_getter_scope() {
    let output = peek(&["-k", "getter", "displayName", "tests/fixtures/dart"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "UserService.displayName" && r.kind == "getter")
    );
}

#[test]
fn peek_for_dart_mixin_method_scope() {
    let output = peek(&["-k", "method", "log", "tests/fixtures/dart"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "Loggable.log" && r.kind == "method")
    );
}

#[test]
fn peek_for_dart_extension_method_scope() {
    let output = peek(&["-k", "method", "repeated", "tests/fixtures/dart"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "StringExt.repeated" && r.kind == "method")
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
    let output = peek(&["-k", "alias", "Callback", "tests/fixtures/dart"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "Callback" && r.kind == "alias")
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

// === Dart function end line tests ===
// Verify that function definitions with multi-line bodies report correct end lines.
// In tree-sitter-dart, function_signature and function_body are siblings,
// so line_range must include the function_body sibling's end position.

#[test]
fn peek_for_dart_class_method_end_line() {
    // greet spans lines 29-31 in the fixture
    let output = peek(&["-k", "method", "greet", "tests/fixtures/dart"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    let def = results
        .iter()
        .find(|r| r.scope == "UserService.greet")
        .unwrap();
    assert_eq!(def.start, 29, "greet should start at line 29");
    assert_eq!(
        def.end, 31,
        "greet should end at line 31 (includes function body)"
    );
}

#[test]
fn peek_for_dart_single_line_functions_unchanged() {
    // Single-line and abstract definitions should have start == end
    let output = peek(&["-k", "callable", "tests/fixtures/dart"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);

    // Abstract method: void process() — line 47, no body
    let abstract_method = results
        .iter()
        .find(|r| r.scope == "BaseProcessor.process")
        .unwrap();
    assert_eq!(
        abstract_method.start, abstract_method.end,
        "abstract method should have start == end, got {}-{}",
        abstract_method.start, abstract_method.end
    );

    // Getter with expression body: String get displayName — line 34
    let getter = results
        .iter()
        .find(|r| r.scope == "UserService.displayName" && r.signature.starts_with("String get"))
        .unwrap();
    assert_eq!(
        getter.start, getter.end,
        "single-line getter should have start == end, got {}-{}",
        getter.start, getter.end
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

// === Dart Field tests ===

#[test]
fn peek_for_dart_class_field() {
    // Product.id is a class field
    let output = peek(&["-k", "field", "id", "tests/fixtures/dart"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "Product.id" && r.kind == "field"),
        "expected field Product.id, got: {results:?}"
    );
}

#[test]
fn peek_for_dart_final_field() {
    // final String name in UserService -> Field
    let output = peek(&["-k", "field", "name", "tests/fixtures/dart"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "UserService.name" && r.kind == "field"),
        "expected field UserService.name, got: {results:?}"
    );
}

#[test]
fn peek_for_dart_field_not_const() {
    // Product.id should not appear as const
    let output = peek(&["-k", "const", "id", "tests/fixtures/dart"]);
    assert_eq!(
        output.status.code(),
        Some(1),
        "field should not match -k const"
    );
}

#[test]
fn peek_for_dart_value_category_includes_field() {
    let output = peek(&["-k", "value", "label", "tests/fixtures/dart"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results.iter().any(|r| r.scope == "Product.label"),
        "expected Product.label in value category, got: {results:?}"
    );
}
