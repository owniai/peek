mod common;

use common::{parse_defs, peek};

// === Scope tests ===

#[test]
fn peek_for_go_method_scope() {
    let output = peek(&["-k", "function", "ValueReceiverMethod", "tests/fixtures/go"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].kind, "function");
    assert_eq!(results[0].scope, "Server.ValueReceiverMethod");
}

#[test]
fn peek_for_go_pointer_method_scope() {
    let output = peek(&[
        "-k",
        "function",
        "PointerReceiverMethod",
        "tests/fixtures/go",
    ]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].scope, "Server.PointerReceiverMethod");
}

#[test]
fn peek_for_go_generic_method_scope() {
    let output = peek(&["-k", "function", "Get", "tests/fixtures/go"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].scope, "Container.Get");
}

#[test]
fn peek_for_go_method_on_defined_type_scope() {
    let output = peek(&["-k", "function", "IsPositive", "tests/fixtures/go"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].scope, "DefinedInt.IsPositive");
}

#[test]
fn peek_for_go_multi_method_same_type() {
    let output = peek(&["-k", "function", "ValueMethod", "tests/fixtures/go"]);
    assert!(output.status.success());
    let results = parse_defs(&String::from_utf8_lossy(&output.stdout));
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].scope, "MultiMethod.ValueMethod");

    let output = peek(&["-k", "function", "PtrMethod", "tests/fixtures/go"]);
    assert!(output.status.success());
    let results = parse_defs(&String::from_utf8_lossy(&output.stdout));
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].scope, "MultiMethod.PtrMethod");
}

#[test]
fn peek_for_go_struct_scope() {
    let output = peek(&["-k", "struct", "Server", "tests/fixtures/go"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].kind, "struct");
    assert_eq!(results[0].scope, "Server");
}

#[test]
fn peek_for_go_interface_scope() {
    let output = peek(&["-k", "interface", "Reader", "tests/fixtures/go"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].kind, "interface");
    assert_eq!(results[0].scope, "Reader");
}

#[test]
fn peek_for_go_type_alias_scope() {
    let output = peek(&["-k", "type", "AliasInt", "tests/fixtures/go"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].kind, "type");
    assert_eq!(results[0].scope, "AliasInt");
}

#[test]
fn peek_for_go_const_scope() {
    let output = peek(&["-k", "const", "StatusOK", "tests/fixtures/go"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].kind, "const");
    assert_eq!(results[0].scope, "StatusOK");
}

// === Edge case tests ===

#[test]
fn peek_for_go_grouped_type_scope() {
    let output = peek(&["GroupedPoint", "tests/fixtures/go"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].kind, "struct");
    assert_eq!(results[0].scope, "GroupedPoint");
    // Grouped struct signature should not include "type (" prefix
    let grouped_point = results.iter().find(|d| d.scope == "GroupedPoint").unwrap();
    assert!(!grouped_point.signature.contains("type ("));
}

#[test]
fn peek_for_go_grouped_interface_signature() {
    let output = peek(&["GroupedHandler", "tests/fixtures/go/sample.go"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    let iface = results.iter().find(|d| d.kind == "interface").unwrap();
    assert!(iface.signature.starts_with("GroupedHandler interface"));
    assert!(!iface.signature.contains("type ("));
}

#[test]
fn peek_for_go_grouped_type_def_signature() {
    let output = peek(&["GroupedInt", "tests/fixtures/go/sample.go"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].kind, "type");
    assert!(results[0].signature.starts_with("GroupedInt int"));
    assert!(!results[0].signature.contains("type ("));
}

#[test]
fn peek_for_go_generic_func() {
    let output = peek(&["-k", "function", "GenericFunc", "tests/fixtures/go"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].scope, "GenericFunc");
    assert!(results[0].signature.contains("[T any]"));
}

#[test]
fn peek_for_go_typed_const() {
    let output = peek(&["-k", "const", "TypedConst", "tests/fixtures/go"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert_eq!(results.len(), 1);
    let def = &results[0];
    assert_eq!(def.kind, "const");
    assert_eq!(def.scope, "TypedConst");
}

#[test]
fn peek_for_go_multiple_consts_in_group() {
    let output = peek(&["-k", "const", "StatusError", "tests/fixtures/go"]);
    assert!(output.status.success());
    let results = parse_defs(&String::from_utf8_lossy(&output.stdout));
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].scope, "StatusError");
}

// === False positive tests ===

#[test]
fn peek_for_go_no_false_positive_var() {
    let output = peek(&["GlobalVar", "tests/fixtures/go"]);
    assert_eq!(
        output.status.code(),
        Some(1),
        "var declarations should not be matched"
    );
}

#[test]
fn peek_for_go_no_false_positive_import() {
    let output = peek(&["fmt", "tests/fixtures/go"]);
    assert_eq!(
        output.status.code(),
        Some(1),
        "import statements should not be matched"
    );
}

// === Top-level definition kind+scope tests ===

#[test]
fn peek_for_go_top_level_func() {
    let output = peek(&["-k", "function", "simpleFunc", "tests/fixtures/go"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].kind, "function");
    assert_eq!(results[0].scope, "simpleFunc");
}

#[test]
fn peek_for_go_point_struct() {
    let output = peek(&["-w", "-k", "struct", "Point", "tests/fixtures/go"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].kind, "struct");
    assert_eq!(results[0].scope, "Point");
}

#[test]
fn peek_for_go_readwriter_interface() {
    let output = peek(&["-k", "interface", "ReadWriter", "tests/fixtures/go"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].kind, "interface");
    assert_eq!(results[0].scope, "ReadWriter");
}

#[test]
fn peek_for_go_iota_consts() {
    // Red: iota const with explicit value
    let output = peek(&["-k", "const", "Red", "tests/fixtures/go"]);
    assert!(output.status.success());
    let results = parse_defs(&String::from_utf8_lossy(&output.stdout));
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].kind, "const");
    assert_eq!(results[0].scope, "Red");
}

// === Function-body definitions should NOT be extracted ===

#[test]
fn peek_for_go_function_body_const_not_extracted() {
    let output = peek(&["-k", "const", "LocalConst", "tests/fixtures/go"]);
    assert_eq!(
        output.status.code(),
        Some(1),
        "Expected no results for function-body const"
    );
}

#[test]
fn peek_for_go_function_body_type_not_extracted() {
    let output = peek(&["-k", "type", "LocalType", "tests/fixtures/go"]);
    assert_eq!(
        output.status.code(),
        Some(1),
        "Expected no results for function-body type"
    );
}
