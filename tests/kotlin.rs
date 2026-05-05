mod common;

use common::{parse_defs, peek};

// === Kotlin scope verification & edge-case tests ===

#[test]
fn peek_for_kotlin_nested_class_scope() {
    let output = peek(&["-k", "class", "Inner", "tests/fixtures/kotlin"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "Outer.Inner" && r.kind == "class")
    );
}

#[test]
fn peek_for_kotlin_nested_interface_scope() {
    let output = peek(&["-k", "interface", "Handler", "tests/fixtures/kotlin"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "Outer.Handler" && r.kind == "interface")
    );
}

#[test]
fn peek_for_kotlin_nested_object_scope() {
    let output = peek(&["-k", "object", "Cache", "tests/fixtures/kotlin"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "Outer.Cache" && r.kind == "object")
    );
}

#[test]
fn peek_for_kotlin_nested_enum_scope() {
    let output = peek(&["-k", "enum", "Priority", "tests/fixtures/kotlin"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "Outer.Priority" && r.kind == "enum")
    );
}

#[test]
fn peek_for_kotlin_deeply_nested_scope() {
    let output = peek(&["-k", "class", "Config", "tests/fixtures/kotlin"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    // Container.Builder.Config
    assert!(
        results
            .iter()
            .any(|r| r.scope == "Container.Builder.Config" && r.kind == "class")
    );
}

#[test]
fn peek_for_kotlin_method_scope() {
    let output = peek(&["-k", "method", "greet", "tests/fixtures/kotlin"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "SimpleClass.greet" && r.kind == "method")
    );
}

#[test]
fn peek_for_kotlin_sealed_class_nested_scope() {
    let output = peek(&["-k", "class", "Circle", "tests/fixtures/kotlin"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "Shape.Circle" && r.kind == "class")
    );
}

#[test]
fn peek_for_kotlin_sealed_interface_nested_scope() {
    let output = peek(&["-k", "class", "Literal", "tests/fixtures/kotlin"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "Node.Literal" && r.kind == "class")
    );
}

#[test]
fn peek_for_kotlin_object_const_scope() {
    let output = peek(&["-k", "const", "PI", "tests/fixtures/kotlin"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "MathUtils.PI" && r.kind == "const")
    );
}

#[test]
fn peek_for_kotlin_companion_object_const() {
    let output = peek(&["-k", "const", "DEFAULT_TIMEOUT", "tests/fixtures/kotlin"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "Config.Defaults.DEFAULT_TIMEOUT" && r.kind == "const")
    );
}

#[test]
fn peek_for_kotlin_multiple_consts() {
    let output = peek(&["-k", "const", "APP_VERSION", "tests/fixtures/kotlin"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "APP_VERSION" && r.kind == "const")
    );
    let output2 = peek(&["-k", "const", "MAX_RETRIES", "tests/fixtures/kotlin"]);
    let stdout2 = String::from_utf8_lossy(&output2.stdout);
    assert!(output2.status.success());
    let results2 = parse_defs(&stdout2);
    assert!(
        results2
            .iter()
            .any(|r| r.scope == "MAX_RETRIES" && r.kind == "const")
    );
}

#[test]
fn peek_for_kotlin_package_not_matched_by_default_kinds() {
    // package should only be extracted when -k package is specified
    // Default search (no -k filter) includes all supported kinds for Kotlin, including Package
    // So searching "com" now matches the package declaration. Instead, verify that
    // package does not match specific non-package kind filters.
    let output = peek(&[
        "-k",
        "class",
        "com.example.testapp",
        "tests/fixtures/kotlin",
    ]);
    assert_eq!(
        output.status.code(),
        Some(1),
        "package should not match -k class"
    );
}

// --- Kotlin extension function ---

#[test]
fn peek_for_kotlin_extension_function_found() {
    let output = peek(&["-k", "function", "isEmail", "tests/fixtures/kotlin"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    // Extension functions are top-level (scope is empty), so kind is "function"
    let results = parse_defs(&stdout);
    assert!(
        !results.is_empty(),
        "extension function 'isEmail' not found. Got: {}",
        stdout
    );
}

#[test]
fn peek_for_kotlin_extension_function_with_modifier_found() {
    let output = peek(&["-k", "function", "isPositive", "tests/fixtures/kotlin"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    // Extension functions are top-level (scope is empty), so kind is "function"
    let results = parse_defs(&stdout);
    assert!(
        !results.is_empty(),
        "extension function 'isPositive' not found. Got: {}",
        stdout
    );
}

#[test]
fn peek_for_kotlin_regular_function_still_works() {
    // Verify regular (non-extension) functions are still found correctly
    let output = peek(&["-k", "function", "regularFunction", "tests/fixtures/kotlin"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert_eq!(results.len(), 1);
    assert!(stdout.contains("regularFunction"));
}

// === Kotlin constructor tests ===

#[test]
fn peek_for_kotlin_primary_constructor() {
    let output = peek(&["-k", "constructor", "SimpleClass", "tests/fixtures/kotlin"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "SimpleClass" && r.kind == "constructor"),
        "expected SimpleClass primary constructor, got: {results:?}"
    );
}

#[test]
fn peek_for_kotlin_secondary_constructor() {
    let output = peek(&["-k", "constructor", "Person", "tests/fixtures/kotlin"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    // Should find both primary and secondary constructors
    assert!(
        results
            .iter()
            .any(|r| r.scope == "Person" && r.kind == "constructor"),
        "expected Person constructor(s), got: {results:?}"
    );
}

#[test]
fn peek_for_kotlin_constructor_not_matched_by_function() {
    let output = peek(&["-k", "function", "SimpleClass", "tests/fixtures/kotlin"]);
    assert_eq!(
        output.status.code(),
        Some(1),
        "constructor should not match -k function"
    );
}

// === Kotlin Property tests ===

#[test]
fn peek_for_kotlin_body_property() {
    // var displayName in UserProfile class body
    let output = peek(&["-k", "property", "displayName", "tests/fixtures/kotlin"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "UserProfile.displayName" && r.kind == "property"),
        "expected property UserProfile.displayName, got: {results:?}"
    );
}

#[test]
fn peek_for_kotlin_val_body_property() {
    // val isActive in UserProfile class body
    let output = peek(&["-k", "property", "isActive", "tests/fixtures/kotlin"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "UserProfile.isActive" && r.kind == "property"),
        "expected property UserProfile.isActive, got: {results:?}"
    );
}

#[test]
fn peek_for_kotlin_class_parameter_val() {
    // val name in SimpleClass constructor -> Property
    let output = peek(&["-k", "property", "name", "tests/fixtures/kotlin"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "SimpleClass.name" && r.kind == "property"),
        "expected property SimpleClass.name, got: {results:?}"
    );
}

#[test]
fn peek_for_kotlin_class_parameter_var() {
    // var mutableParam in MixedParams constructor -> Property
    let output = peek(&["-k", "property", "mutableParam", "tests/fixtures/kotlin"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "MixedParams.mutableParam" && r.kind == "property"),
        "expected property MixedParams.mutableParam, got: {results:?}"
    );
}

#[test]
fn peek_for_kotlin_plain_param_not_property() {
    // plainParam (no val/var) in MixedParams constructor -> should NOT be Property
    let output = peek(&["-k", "property", "plainParam", "tests/fixtures/kotlin"]);
    assert_eq!(
        output.status.code(),
        Some(1),
        "plainParam should not be property"
    );
}

#[test]
fn peek_for_kotlin_const_not_property() {
    // const val APP_NAME should be Const, not Property
    let output = peek(&["-k", "property", "APP_NAME", "tests/fixtures/kotlin"]);
    assert_eq!(
        output.status.code(),
        Some(1),
        "const val should not be property"
    );
}

#[test]
fn peek_for_kotlin_value_category_includes_property() {
    let output = peek(&["-k", "value", "displayName", "tests/fixtures/kotlin"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results.iter().any(|r| r.scope == "UserProfile.displayName"),
        "expected displayName in value category, got: {results:?}"
    );
}

// === Package extraction tests ===

#[test]
fn peek_for_kotlin_package_extraction() {
    let output = peek(&[
        "-k",
        "package",
        "-e",
        "com.example.testapp",
        "tests/fixtures/kotlin",
    ]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].kind, "package");
    assert_eq!(results[0].scope, "com.example.testapp");
    assert_eq!(
        results[0].start, 1,
        "package declaration should be on line 1 of comprehensive.kt"
    );
}

#[test]
fn peek_for_kotlin_package_signature() {
    let output = peek(&[
        "-k",
        "package",
        "-e",
        "com.example.testapp",
        "tests/fixtures/kotlin",
    ]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert_eq!(results.len(), 1);
    assert!(results[0].signature.contains("package com.example.testapp"));
}

#[test]
fn peek_for_kotlin_package_kind_excludes_class() {
    let output = peek(&[
        "-k",
        "class",
        "-e",
        "com.example.testapp",
        "tests/fixtures/kotlin",
    ]);
    assert_eq!(
        output.status.code(),
        Some(1),
        "package should not match -k class"
    );
}
