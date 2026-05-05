mod common;

use common::{parse_defs, peek};

// === Scope tests ===

#[test]
fn peek_for_go_method_scope() {
    let output = peek(&["-k", "method", "ValueReceiverMethod", "tests/fixtures/go"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].kind, "method");
    assert_eq!(results[0].scope, "Server.ValueReceiverMethod");
}

#[test]
fn peek_for_go_pointer_method_scope() {
    let output = peek(&["-k", "method", "PointerReceiverMethod", "tests/fixtures/go"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].kind, "method");
    assert_eq!(results[0].scope, "Server.PointerReceiverMethod");
}

#[test]
fn peek_for_go_generic_method_scope() {
    let output = peek(&["-k", "method", "Get", "tests/fixtures/go"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].kind, "method");
    assert_eq!(results[0].scope, "Container.Get");
}

#[test]
fn peek_for_go_method_on_defined_type_scope() {
    let output = peek(&["-k", "method", "IsPositive", "tests/fixtures/go"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].kind, "method");
    assert_eq!(results[0].scope, "DefinedInt.IsPositive");
}

#[test]
fn peek_for_go_multi_method_same_type() {
    let output = peek(&["-k", "method", "ValueMethod", "tests/fixtures/go"]);
    assert!(output.status.success());
    let results = parse_defs(&String::from_utf8_lossy(&output.stdout));
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].kind, "method");
    assert_eq!(results[0].scope, "MultiMethod.ValueMethod");

    let output = peek(&["-k", "method", "PtrMethod", "tests/fixtures/go"]);
    assert!(output.status.success());
    let results = parse_defs(&String::from_utf8_lossy(&output.stdout));
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].kind, "method");
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
    let output = peek(&["-k", "alias", "AliasInt", "tests/fixtures/go"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].kind, "alias");
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

// === Interface method is Method kind ===

#[test]
fn peek_for_go_interface_method_is_method_kind() {
    let output = peek(&["-k", "method", "Read", "tests/fixtures/go"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].kind, "method");
    assert_eq!(results[0].scope, "Reader.Read");
}

// === Edge case tests ===

#[test]
fn peek_for_go_grouped_type_scope() {
    let output = peek(&["-k", "struct", "GroupedPoint", "tests/fixtures/go"]);
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
    assert_eq!(results[0].kind, "alias");
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
fn peek_for_go_function_kind_excludes_methods() {
    let output = peek(&["-k", "function", "ValueReceiverMethod", "tests/fixtures/go"]);
    assert_eq!(
        output.status.code(),
        Some(1),
        "-k function should not match methods"
    );
}

#[test]
fn peek_for_go_callable_includes_functions_and_methods() {
    let output = peek(&["-k", "callable", "simpleFunc", "tests/fixtures/go"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].kind, "function");
    assert_eq!(results[0].scope, "simpleFunc");

    let output2 = peek(&["-k", "callable", "ValueReceiverMethod", "tests/fixtures/go"]);
    let stdout2 = String::from_utf8_lossy(&output2.stdout);
    assert!(output2.status.success());
    let results2 = parse_defs(&stdout2);
    assert_eq!(results2.len(), 1);
    assert_eq!(results2[0].kind, "method");
    assert_eq!(results2[0].scope, "Server.ValueReceiverMethod");
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
    let output = peek(&["-k", "alias", "LocalType", "tests/fixtures/go"]);
    assert_eq!(
        output.status.code(),
        Some(1),
        "Expected no results for function-body type"
    );
}

// === Field tests ===

#[test]
fn peek_for_go_struct_field() {
    let output = peek(&["-k", "field", "Host", "tests/fixtures/go"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].kind, "field");
    assert_eq!(results[0].scope, "Server.Host");
}

#[test]
fn peek_for_go_struct_field_y() {
    let output = peek(&["-k", "field", "-w", "Y", "tests/fixtures/go"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    // Point has Y, Pair has Value
    assert!(!results.is_empty());
    assert!(results.iter().any(|r| r.scope == "Point.Y"));
}

#[test]
fn peek_for_go_field_kind_excludes_struct() {
    // "Host" is a field name, searching with -k struct should return nothing
    let output = peek(&["-k", "struct", "Host", "tests/fixtures/go"]);
    assert_eq!(
        output.status.code(),
        Some(1),
        "field name should not match -k struct"
    );
}

#[test]
fn peek_for_go_embedded_field_not_matched() {
    // Embedded struct fields don't have an explicit name (field_identifier).
    // "data" is a field in MultiMethod struct, but "Server" is an embedded field
    // in Embedded struct with no explicit name. Verify "data" IS found (proving fields work),
    // and the field count for "Embedded" struct is only 1 (the "Name" field, not the "Server" embedded).
    let output = peek(&["-k", "field", "-w", "Name", "tests/fixtures/go"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    // "Name" field in Embedded struct should exist
    assert!(results.iter().any(|r| r.scope == "Embedded.Name"));
    // Should NOT have "Embedded.Server" -- embedded fields have no name
    assert!(
        !results.iter().any(|r| r.scope == "Embedded.Server"),
        "embedded field should not have explicit scope, got: {results:?}"
    );
}

// === Value category expansion ===

#[test]
fn peek_for_go_value_category_includes_field() {
    let output = peek(&["-k", "value", "Host", "tests/fixtures/go"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(!results.is_empty());
    assert!(results.iter().any(|r| r.kind == "field"));
}

// === Package extraction tests ===

#[test]
fn peek_for_go_package_extraction() {
    let output = peek(&["-k", "package", "sample", "tests/fixtures/go"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].kind, "package");
    assert_eq!(results[0].scope, "sample");
    assert_eq!(
        results[0].start, 2,
        "package declaration should be on line 2 of sample.go"
    );
}

#[test]
fn peek_for_go_package_signature() {
    let output = peek(&["-k", "package", "sample", "tests/fixtures/go"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert_eq!(results.len(), 1);
    assert!(results[0].signature.contains("package sample"));
}

#[test]
fn peek_for_go_package_kind_excludes_struct() {
    let output = peek(&["-k", "struct", "sample", "tests/fixtures/go"]);
    assert_eq!(
        output.status.code(),
        Some(1),
        "package should not match -k struct"
    );
}
