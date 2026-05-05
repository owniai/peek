mod common;

use common::{parse_defs, peek};

// === C integration tests ===

#[test]
fn peek_for_c_static_function() {
    let output = peek(&["-k", "function", "helper", "tests/fixtures/c"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    assert!(stdout.contains("helper"));
    let results = parse_defs(&stdout);
    assert!(results.iter().any(|r| r.signature.contains("static")));
}

#[test]
fn peek_for_c_pointer_return_function() {
    let output = peek(&["-k", "function", "dup_str", "tests/fixtures/c"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "dup_str" && r.kind == "function")
    );
}

// --- Supplementary: top-level definition kind+scope validation ---

#[test]
fn peek_for_c_top_level_function_scope() {
    let output = peek(&["-k", "function", "process", "tests/fixtures/c"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "process" && r.kind == "function")
    );
}

#[test]
fn peek_for_c_struct_scope() {
    let output = peek(&["-k", "struct", "Point", "tests/fixtures/c"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "Point" && r.kind == "struct")
    );
}

#[test]
fn peek_for_c_enum_scope() {
    let output = peek(&["-k", "enum", "Color", "tests/fixtures/c"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "Color" && r.kind == "enum")
    );
}

#[test]
fn peek_for_c_typedef_scope() {
    let output = peek(&["-k", "alias", "StatusCode", "tests/fixtures/c"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "StatusCode" && r.kind == "alias")
    );
}

#[test]
fn peek_for_c_const_scope() {
    let output = peek(&["-k", "const", "MAX_SIZE", "tests/fixtures/c"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "MAX_SIZE" && r.kind == "const")
    );
}

#[test]
fn peek_for_c_static_const_scope() {
    let output = peek(&["-k", "const", "VERSION", "tests/fixtures/c"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "VERSION" && r.kind == "const")
    );
}

// NOTE: No macro test — neither comprehensive.c nor union_bug.c contains #define directives.

#[test]
fn peek_for_c_union_scope() {
    let output = peek(&["-k", "union", "Value", "tests/fixtures/c"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "Value" && r.kind == "union")
    );
}

#[test]
fn peek_for_c_union_not_matched_by_struct() {
    let output = peek(&["-k", "struct", "Value", "tests/fixtures/c"]);
    assert_eq!(
        output.status.code(),
        Some(1),
        "union should not match struct kind filter"
    );
}

// === Function-body definitions should NOT be extracted ===

#[test]
fn peek_for_c_function_body_const_not_extracted() {
    let output = peek(&["-k", "const", "LOCAL_CONST", "tests/fixtures/c"]);
    assert_eq!(
        output.status.code(),
        Some(1),
        "Expected no results for function-body const"
    );
}

// === Field tests ===

#[test]
fn peek_for_c_struct_field() {
    let output = peek(&["-k", "field", "-w", "x", "tests/fixtures/c"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(!results.is_empty());
    assert!(
        results
            .iter()
            .any(|r| r.kind == "field" && r.scope == "Point::x")
    );
}

#[test]
fn peek_for_c_struct_field_timeout() {
    let output = peek(&["-k", "field", "timeout", "tests/fixtures/c"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(!results.is_empty());
    assert!(
        results
            .iter()
            .any(|r| r.kind == "field" && r.scope == "Config::timeout")
    );
}

#[test]
fn peek_for_c_field_kind_excludes_struct() {
    // "timeout" is a field in Config struct, searching with -k struct should return nothing
    let output = peek(&["-k", "struct", "timeout", "tests/fixtures/c"]);
    assert_eq!(
        output.status.code(),
        Some(1),
        "field name should not match -k struct"
    );
}

// === Value category expansion ===

#[test]
fn peek_for_c_value_category_includes_field() {
    let output = peek(&["-k", "value", "-w", "x", "tests/fixtures/c"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(!results.is_empty());
    assert!(results.iter().any(|r| r.kind == "field"));
}

// === Static tests ===

#[test]
fn peek_for_c_file_scope_static() {
    let output = peek(&["-k", "static", "-w", "file_count", "tests/fixtures/c"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(!results.is_empty());
    assert!(
        results
            .iter()
            .any(|r| r.kind == "static" && r.scope == "file_count")
    );
}

#[test]
fn peek_for_c_static_const_is_const_not_static() {
    // `static const int VERSION = 2;` should be Const, not Static
    let output = peek(&["-k", "static", "-w", "VERSION", "tests/fixtures/c"]);
    assert_eq!(
        output.status.code(),
        Some(1),
        "static const should be Const, not Static"
    );
}

#[test]
fn peek_for_c_static_pointer_var() {
    let output = peek(&["-k", "static", "-w", "file_name", "tests/fixtures/c"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(!results.is_empty());
    assert!(
        results
            .iter()
            .any(|r| r.kind == "static" && r.scope == "file_name")
    );
}

#[test]
fn peek_for_c_value_category_includes_static() {
    let output = peek(&["-k", "value", "-w", "file_count", "tests/fixtures/c"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(!results.is_empty());
    assert!(results.iter().any(|r| r.kind == "static"));
}
