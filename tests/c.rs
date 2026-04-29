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
    let output = peek(&["-k", "type", "StatusCode", "tests/fixtures/c"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "StatusCode" && r.kind == "type")
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
