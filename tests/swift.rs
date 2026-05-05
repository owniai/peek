mod common;

use common::{parse_defs, peek};

// === Swift scope verification tests ===

#[test]
fn peek_for_swift_nested_struct_scope() {
    let output = peek(&["-k", "struct", "Config", "tests/fixtures/swift"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "NetworkManager.Config" && r.kind == "struct")
    );
}

#[test]
fn peek_for_swift_nested_enum_scope() {
    let output = peek(&["-k", "enum", "Status", "tests/fixtures/swift"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "NetworkManager.Status" && r.kind == "enum")
    );
}

#[test]
fn peek_for_swift_method_scope() {
    let output = peek(&["-k", "method", "connect", "tests/fixtures/swift"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "NetworkManager.connect" && r.kind == "method")
    );
}

#[test]
fn peek_for_swift_static_method() {
    let output = peek(&["-k", "method", "create", "tests/fixtures/swift"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "NetworkManager.create" && r.kind == "method")
    );
}

#[test]
fn peek_for_swift_extension_function_scope() {
    let output = peek(&["-k", "method", "repeated", "tests/fixtures/swift"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "String.repeated" && r.kind == "method")
    );
}

#[test]
fn peek_for_swift_extension_const_scope() {
    let output = peek(&["-k", "const", "defaultEncoding", "tests/fixtures/swift"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "String.defaultEncoding" && r.kind == "const")
    );
}

#[test]
fn peek_for_swift_actor_method_scope() {
    let output = peek(&["-k", "method", "enqueue", "tests/fixtures/swift"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "MessageQueue.enqueue" && r.kind == "method")
    );
}

#[test]
fn peek_for_swift_class_const_scope() {
    let output = peek(&["-k", "const", "maxConnections", "tests/fixtures/swift"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "NetworkManager.maxConnections" && r.kind == "const")
    );
}

#[test]
fn peek_for_swift_nested_enum_method_scope() {
    let output = peek(&["-k", "method", "describe", "tests/fixtures/swift"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "NetworkManager.Status.describe" && r.kind == "method")
    );
}

#[test]
fn peek_for_swift_protocol_method_scope() {
    let output = peek(&["-k", "method", "serialize", "tests/fixtures/swift"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "Serializable.serialize" && r.kind == "method")
    );
}

#[test]
fn peek_for_swift_no_false_positive() {
    // Comments should never be extracted
    let output = peek(&["comprehensive", "tests/fixtures/swift"]);
    assert_eq!(
        output.status.code(),
        Some(1),
        "expected no match for comment text"
    );
}

// === Swift top-level kind+scope verification tests ===

#[test]
fn peek_for_swift_top_level_class() {
    let output = peek(&["-k", "class", "NetworkManager", "tests/fixtures/swift"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "NetworkManager" && r.kind == "class")
    );
}

#[test]
fn peek_for_swift_top_level_struct() {
    let output = peek(&["-k", "struct", "Point", "tests/fixtures/swift"]);
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
fn peek_for_swift_top_level_enum() {
    let output = peek(&["-k", "enum", "Direction", "tests/fixtures/swift"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "Direction" && r.kind == "enum")
    );
}

#[test]
fn peek_for_swift_top_level_protocol() {
    let output = peek(&["-k", "protocol", "Serializable", "tests/fixtures/swift"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "Serializable" && r.kind == "protocol")
    );
}

#[test]
fn peek_for_swift_top_level_typealias() {
    let output = peek(&["-k", "alias", "CompletionHandler", "tests/fixtures/swift"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "CompletionHandler" && r.kind == "alias")
    );
}

#[test]
fn peek_for_swift_top_level_const() {
    let output = peek(&["-k", "const", "MAX_RETRIES", "tests/fixtures/swift"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "MAX_RETRIES" && r.kind == "const")
    );
}

#[test]
fn peek_for_swift_top_level_function() {
    let output = peek(&["-k", "function", "processRequest", "tests/fixtures/swift"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "processRequest" && r.kind == "function")
    );
}

// === Swift constructor tests ===

#[test]
fn peek_for_swift_init_as_constructor() {
    let output = peek(&["-k", "constructor", "init", "tests/fixtures/swift"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "NetworkManager.init" && r.kind == "constructor"),
        "expected NetworkManager.init as constructor, got: {results:?}"
    );
}

#[test]
fn peek_for_swift_init_not_matched_by_function() {
    let output = peek(&["-k", "function", "init", "tests/fixtures/swift"]);
    assert_eq!(
        output.status.code(),
        Some(1),
        "init should not match -k function"
    );
}

#[test]
fn peek_for_swift_actor() {
    let output = peek(&["-k", "actor", "MessageQueue", "tests/fixtures/swift"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "MessageQueue" && r.kind == "actor")
    );
}

#[test]
fn peek_for_swift_extension() {
    let output = peek(&["-k", "extension", "String", "tests/fixtures/swift"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "String" && r.kind == "extension")
    );
}

// === Swift Property tests ===

#[test]
fn peek_for_swift_var_property() {
    // var activeConnections in NetworkManager class
    let output = peek(&[
        "-k",
        "property",
        "activeConnections",
        "tests/fixtures/swift",
    ]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "NetworkManager.activeConnections" && r.kind == "property"),
        "expected property NetworkManager.activeConnections, got: {results:?}"
    );
}

#[test]
fn peek_for_swift_struct_var_property() {
    // var x in Point struct
    let output = peek(&["-k", "property", "x", "tests/fixtures/swift"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "Point.x" && r.kind == "property"),
        "expected property Point.x, got: {results:?}"
    );
}

#[test]
fn peek_for_swift_let_is_const_not_property() {
    // let maxConnections should be Const, not Property
    let output = peek(&["-k", "const", "maxConnections", "tests/fixtures/swift"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "NetworkManager.maxConnections" && r.kind == "const"),
        "expected const NetworkManager.maxConnections, got: {results:?}"
    );
}

#[test]
fn peek_for_swift_var_not_matched_by_const() {
    // var activeConnections should NOT appear as const
    let output = peek(&["-k", "const", "activeConnections", "tests/fixtures/swift"]);
    assert_eq!(
        output.status.code(),
        Some(1),
        "expected no match for var as const"
    );
}

#[test]
fn peek_for_swift_value_category_includes_property() {
    let output = peek(&["-k", "value", "activeConnections", "tests/fixtures/swift"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "NetworkManager.activeConnections"),
        "expected activeConnections in value category, got: {results:?}"
    );
}
