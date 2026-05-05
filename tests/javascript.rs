mod common;

use common::{parse_defs, peek};

// === Scope tests ===

#[test]
fn peek_for_js_method_scope() {
    let output = peek(&["-k", "method", "regularMethod", "tests/fixtures/js"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].kind, "method");
    assert_eq!(results[0].scope, "WithMethods.regularMethod");
}

#[test]
fn peek_for_js_nested_function_not_extracted() {
    let output = peek(&["-k", "function", "innerFunc", "tests/fixtures/js"]);
    // Function-body definitions should NOT be extracted
    assert_eq!(
        output.status.code(),
        Some(1),
        "Expected no results for function-body definition"
    );
}

#[test]
fn peek_for_js_object_literal_method_scope() {
    let output = peek(&["-k", "method", "methodOne", "tests/fixtures/js"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].scope, "obj.methodOne");
}

#[test]
fn peek_for_js_class_scope() {
    let output = peek(&["-k", "class", "SimpleClass", "tests/fixtures/js"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].kind, "class");
    assert_eq!(results[0].scope, "SimpleClass");
}

#[test]
fn peek_for_js_const_scope() {
    let output = peek(&["-k", "const", "constVar", "tests/fixtures/js"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].kind, "const");
    assert_eq!(results[0].scope, "constVar");
}

// === Edge case tests ===

#[test]
fn peek_for_js_exported_func_signature() {
    let output = peek(&["-k", "function", "exportedFunc", "tests/fixtures/js"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert_eq!(results.len(), 1);
    assert!(
        results[0].signature.contains("export"),
        "exported func signature should include 'export', got: {}",
        results[0].signature
    );
}

#[test]
fn peek_for_js_generator_func() {
    let output = peek(&["-k", "function", "genFunc", "tests/fixtures/js"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert_eq!(results.len(), 1);
    let def = &results[0];
    assert_eq!(def.kind, "function");
    assert_eq!(def.scope, "genFunc");
}

#[test]
fn peek_for_js_arrow_const() {
    let output = peek(&["-w", "-k", "function", "arrowFunc", "tests/fixtures/js"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].kind, "function");
    assert_eq!(results[0].scope, "arrowFunc");
}

// === False positive tests ===

#[test]
fn peek_for_js_no_false_positive_let_var() {
    let output = peek(&["letVar", "tests/fixtures/js"]);
    assert_eq!(
        output.status.code(),
        Some(1),
        "let declarations should not be matched"
    );
}

#[test]
fn peek_for_js_no_false_positive_base_class() {
    let output = peek(&["-k", "class", "Base", "tests/fixtures/js"]);
    assert_eq!(
        output.status.code(),
        Some(1),
        "undefined base class should not be matched"
    );
}

// Note: tree-sitter-javascript 0.25.0 does not support public_field_definition
// (class fields like `x;` or `x = 1;` are not parsed as distinct nodes).
// JavaScript Field extraction is NOT supported.
