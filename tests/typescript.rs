mod common;

use common::{parse_defs, peek};

// === Scope tests ===

#[test]
fn peek_for_ts_method_scope() {
    let output = peek(&["-k", "function", "regularMethod", "tests/fixtures/ts"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].kind, "function");
    assert_eq!(results[0].scope, "MethodClass.regularMethod");
}

#[test]
fn peek_for_ts_nested_function_not_extracted() {
    let output = peek(&["-k", "function", "innerFunc", "tests/fixtures/ts"]);
    // Function-body definitions should NOT be extracted
    assert_eq!(
        output.status.code(),
        Some(1),
        "Expected no results for function-body definition"
    );
}

#[test]
fn peek_for_ts_nested_interface_not_extracted() {
    let output = peek(&["-k", "interface", "LocalInterface", "tests/fixtures/ts"]);
    // Function-body definitions should NOT be extracted
    assert_eq!(
        output.status.code(),
        Some(1),
        "Expected no results for function-body definition"
    );
}

#[test]
fn peek_for_ts_nested_enum_not_extracted() {
    let output = peek(&["-k", "enum", "LocalEnum", "tests/fixtures/ts"]);
    // Function-body definitions should NOT be extracted
    assert_eq!(
        output.status.code(),
        Some(1),
        "Expected no results for function-body definition"
    );
}

#[test]
fn peek_for_ts_class_scope() {
    let output = peek(&["-k", "class", "SimpleClass", "tests/fixtures/ts"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].kind, "class");
    assert_eq!(results[0].scope, "SimpleClass");
}

#[test]
fn peek_for_ts_interface_scope() {
    let output = peek(&["-k", "interface", "SimpleInterface", "tests/fixtures/ts"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].kind, "interface");
    assert_eq!(results[0].scope, "SimpleInterface");
}

#[test]
fn peek_for_ts_type_alias_scope() {
    let output = peek(&["-k", "type", "SimpleType", "tests/fixtures/ts"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].kind, "type");
    assert_eq!(results[0].scope, "SimpleType");
}

#[test]
fn peek_for_ts_enum_scope() {
    let output = peek(&["-k", "enum", "SimpleEnum", "tests/fixtures/ts"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].kind, "enum");
    assert_eq!(results[0].scope, "SimpleEnum");
}

#[test]
fn peek_for_ts_const_scope() {
    let output = peek(&["-k", "const", "simpleConst", "tests/fixtures/ts"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].kind, "const");
    assert_eq!(results[0].scope, "simpleConst");
}

// === Edge case tests ===

#[test]
fn peek_for_ts_exported_class_signature() {
    let output = peek(&["-k", "class", "ExportedClass", "tests/fixtures/ts"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert_eq!(results.len(), 1);
    assert!(
        results[0].signature.contains("export"),
        "exported class signature should include 'export', got: {}",
        results[0].signature
    );
}

#[test]
fn peek_for_ts_abstract_class() {
    let output = peek(&["-k", "class", "AbstractBase", "tests/fixtures/ts"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].scope, "AbstractBase");
}

#[test]
fn peek_for_ts_generic_func() {
    let output = peek(&["-k", "function", "genericFunc", "tests/fixtures/ts"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].scope, "genericFunc");
}

// === Additional scope/kind tests ===

#[test]
fn peek_for_ts_abstract_class_method_scope() {
    let output = peek(&["-k", "function", "concreteMethod", "tests/fixtures/ts"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].kind, "function");
    assert_eq!(results[0].scope, "AbstractBase.concreteMethod");
}

#[test]
fn peek_for_ts_nested_type_alias_not_extracted() {
    let output = peek(&["-k", "type", "LocalType", "tests/fixtures/ts"]);
    // Function-body definitions should NOT be extracted
    assert_eq!(
        output.status.code(),
        Some(1),
        "Expected no results for function-body definition"
    );
}

#[test]
fn peek_for_ts_nested_const_not_extracted() {
    let output = peek(&["-k", "const", "localConst", "tests/fixtures/ts"]);
    // Function-body definitions should NOT be extracted
    assert_eq!(
        output.status.code(),
        Some(1),
        "Expected no results for function-body definition"
    );
}

#[test]
fn peek_for_ts_exported_const_scope() {
    let output = peek(&["-k", "const", "exportedConst", "tests/fixtures/ts"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].kind, "const");
    assert_eq!(results[0].scope, "exportedConst");
    assert!(
        results[0].signature.contains("export"),
        "exported const signature should include 'export', got: {}",
        results[0].signature
    );
}
