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
    let output = peek(&["-k", "function", "connect", "tests/fixtures/swift"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "NetworkManager.connect" && r.kind == "function")
    );
}

#[test]
fn peek_for_swift_static_method() {
    let output = peek(&["-k", "function", "create", "tests/fixtures/swift"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "NetworkManager.create" && r.kind == "function")
    );
}

#[test]
fn peek_for_swift_extension_function_scope() {
    let output = peek(&["-k", "function", "repeated", "tests/fixtures/swift"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "String.repeated" && r.kind == "function")
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
    let output = peek(&["-k", "function", "enqueue", "tests/fixtures/swift"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "MessageQueue.enqueue" && r.kind == "function")
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
    let output = peek(&["-k", "function", "describe", "tests/fixtures/swift"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "NetworkManager.Status.describe" && r.kind == "function")
    );
}

#[test]
fn peek_for_swift_protocol_method_scope() {
    let output = peek(&["-k", "function", "serialize", "tests/fixtures/swift"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "Serializable.serialize" && r.kind == "function")
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
    let output = peek(&["-k", "type", "CompletionHandler", "tests/fixtures/swift"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "CompletionHandler" && r.kind == "type")
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
