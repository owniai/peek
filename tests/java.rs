mod common;

use common::{parse_defs, peek};

// --- Java throws signature bug verification ---
// BUG: abstract/interface methods with throws clauses lose the throws part
// in their extracted signature. The root cause is in handle_callable()
// (src/parser/java.rs): when a method has no body, it truncates the signature
// at parameters.end_byte(), which cuts off the throws clause.

#[test]
fn peek_java_abstract_method_throws_signature() {
    let output = peek(&["readData", "tests/fixtures/java"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].kind, "method");
    // BUG: The signature should contain "throws IOException" but it is truncated
    assert!(
        stdout.contains("throws"),
        "BUG: abstract method signature missing throws clause. Got: {}",
        stdout
    );
}

#[test]
fn peek_java_interface_method_throws_signature() {
    let output = peek(&["process", "tests/fixtures/java"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].kind, "method");
    // BUG: The signature should contain "throws IllegalArgumentException" but it is truncated
    assert!(
        stdout.contains("throws"),
        "BUG: interface method signature missing throws clause. Got: {}",
        stdout
    );
}

// === Top-level class / interface / enum kind+scope tests ===

#[test]
fn peek_java_top_level_class() {
    let output = peek(&["-k", "class", "MyClass", "tests/fixtures/java"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    // substring matching: "MyClass" also matches inner classes like MyClass.Builder, MyClass.Builder.Config, MyClass.InnerHelper
    assert_eq!(results.len(), 4);
    assert_eq!(results[0].kind, "class");
    assert_eq!(results[0].scope, "MyClass");
}

#[test]
fn peek_java_top_level_interface() {
    let output = peek(&["-k", "interface", "Drawable", "tests/fixtures/java"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].kind, "interface");
    assert_eq!(results[0].scope, "Drawable");
}

#[test]
fn peek_java_top_level_enum() {
    let output = peek(&["-k", "enum", "Status", "tests/fixtures/java"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].kind, "enum");
    assert_eq!(results[0].scope, "Status");
}

#[test]
fn peek_java_abstract_class() {
    let output = peek(&["-k", "class", "Shape", "tests/fixtures/java"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].kind, "class");
    assert_eq!(results[0].scope, "Shape");
}

// === Nested type scope tests ===

#[test]
fn peek_java_static_inner_class() {
    let output = peek(&["-k", "class", "Builder", "tests/fixtures/java"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    // substring matching: "Builder" also matches MyClass.Builder.Config
    assert_eq!(results.len(), 2);
    assert_eq!(results[0].scope, "MyClass.Builder");
}

#[test]
fn peek_java_deeply_nested_class() {
    let output = peek(&["-k", "class", "Config", "tests/fixtures/java"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].scope, "MyClass.Builder.Config");
}

#[test]
fn peek_java_inner_interface() {
    let output = peek(&["-k", "interface", "Serializable", "tests/fixtures/java"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].scope, "MyClass.Serializable");
}

#[test]
fn peek_java_inner_enum() {
    let output = peek(&["-k", "enum", "Priority", "tests/fixtures/java"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].scope, "MyClass.Priority");
}

// === Method scope tests ===

#[test]
fn peek_java_method_scope_in_class() {
    let output = peek(&["-k", "method", "helperMethod", "tests/fixtures/java"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].kind, "method");
    assert_eq!(results[0].scope, "MyClass.InnerHelper.helperMethod");
}

#[test]
fn peek_java_abstract_method_scope() {
    let output = peek(&["-k", "method", "area", "tests/fixtures/java"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].kind, "method");
    assert_eq!(results[0].scope, "Shape.area");
}

#[test]
fn peek_java_enum_constructor_scope() {
    let output = peek(&["-k", "constructor", "Color", "tests/fixtures/java"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    // Constructor only — method getHex excluded by kind filter
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].kind, "constructor");
    assert_eq!(results[0].scope, "Color.Color");
}

#[test]
fn peek_java_enum_method_scope() {
    let output = peek(&["-k", "method", "getHex", "tests/fixtures/java"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].kind, "method");
    assert_eq!(results[0].scope, "Color.getHex");
}

#[test]
fn peek_java_interface_method_scope() {
    let output = peek(&["-k", "method", "render", "tests/fixtures/java"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].kind, "method");
    assert_eq!(results[0].scope, "Renderable.render");
}

#[test]
fn peek_java_interface_default_method_scope() {
    let output = peek(&["-k", "method", "resize", "tests/fixtures/java"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].kind, "method");
    assert_eq!(results[0].scope, "Renderable.resize");
}

#[test]
fn peek_java_interface_static_method_scope() {
    let output = peek(&["-k", "method", "factory", "tests/fixtures/java"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].kind, "method");
    assert_eq!(results[0].scope, "Renderable.factory");
}

#[test]
fn peek_java_static_method_scope() {
    let output = peek(&["-k", "method", "multiply", "tests/fixtures/java"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].kind, "method");
    assert_eq!(results[0].scope, "Calculator.multiply");
}

#[test]
fn peek_java_overloaded_methods() {
    let output = peek(&["-k", "method", "add", "tests/fixtures/java"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert_eq!(results.len(), 2);
    assert!(
        results
            .iter()
            .all(|r| r.scope == "Calculator.add" && r.kind == "method")
    );
}

#[test]
fn peek_java_inner_interface_method_scope() {
    let output = peek(&["-k", "method", "serialize", "tests/fixtures/java"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].kind, "method");
    assert_eq!(results[0].scope, "MyClass.Serializable.serialize");
}

// === Field tests ===

#[test]
fn peek_java_field_scope() {
    let output = peek(&["-k", "field", "-w", "field", "tests/fixtures/java"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(!results.is_empty());
    assert!(
        results
            .iter()
            .any(|r| r.kind == "field" && r.scope == "MyClass.field")
    );
}

#[test]
fn peek_java_inner_class_field_scope() {
    let output = peek(&["-k", "field", "-w", "name", "tests/fixtures/java"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(!results.is_empty());
    assert!(
        results
            .iter()
            .any(|r| r.kind == "field" && r.scope == "MyClass.Builder.name")
    );
}

#[test]
fn peek_java_deeply_nested_field_scope() {
    let output = peek(&["-k", "field", "-w", "debug", "tests/fixtures/java"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(!results.is_empty());
    assert!(
        results
            .iter()
            .any(|r| r.kind == "field" && r.scope == "MyClass.Builder.Config.debug")
    );
}

#[test]
fn peek_java_field_kind_excludes_class() {
    let output = peek(&["-k", "class", "-w", "field", "tests/fixtures/java"]);
    assert_eq!(
        output.status.code(),
        Some(1),
        "field name should not match -k class"
    );
}

#[test]
fn peek_java_value_category_includes_field() {
    let output = peek(&["-k", "value", "-w", "field", "tests/fixtures/java"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(!results.is_empty());
    assert!(results.iter().any(|r| r.kind == "field"));
}

// === Package extraction tests ===

#[test]
fn peek_java_package_extraction() {
    let output = peek(&["-k", "package", "-e", "com.example", "tests/fixtures/java"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    // Both Sample.java and Comprehensive.java have "package com.example;"
    assert!(!results.is_empty());
    assert!(
        results
            .iter()
            .all(|r| r.kind == "package" && r.scope == "com.example")
    );
    assert!(
        results.iter().any(|r| r.start == 2),
        "package declaration should start on line 2"
    );
}

#[test]
fn peek_java_package_signature() {
    let output = peek(&["-k", "package", "-e", "com.example", "tests/fixtures/java"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(!results.is_empty());
    assert!(
        results
            .iter()
            .all(|r| r.signature.contains("package com.example"))
    );
}
