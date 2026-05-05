mod common;

use common::{parse_defs, peek};

// === Lua scope verification & edge-case tests ===

#[test]
fn peek_for_lua_dot_method_scope() {
    let output = peek(&["-k", "function", "square", "tests/fixtures/lua"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "math_utils.square" && r.kind == "function")
    );
}

#[test]
fn peek_for_lua_colon_method_scope() {
    let output = peek(&["-k", "method", "cube", "tests/fixtures/lua"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "math_utils.cube" && r.kind == "method")
    );
}

#[test]
fn peek_for_lua_multi_level_dot_scope() {
    let output = peek(&["-k", "function", "create_user", "tests/fixtures/lua"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "app.models.create_user" && r.kind == "function")
    );
}

#[test]
fn peek_for_lua_nested_function_scope() {
    let output = peek(&["-k", "function", "step1", "tests/fixtures/lua"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "process.step1" && r.kind == "function")
    );
}

#[test]
fn peek_for_lua_deeply_nested_scope() {
    let output = peek(&["-k", "function", "parse_body", "tests/fixtures/lua"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "server.handle_request.parse_body" && r.kind == "function")
    );
}

#[test]
fn peek_for_lua_dot_method_nested_function_scope() {
    let output = peek(&["-k", "function", "parse_file", "tests/fixtures/lua"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "config.load.parse_file" && r.kind == "function")
    );
}

#[test]
fn peek_for_lua_colon_method_nested_function_scope() {
    let output = peek(&["-k", "function", "write_file", "tests/fixtures/lua"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "config.save.write_file" && r.kind == "function")
    );
}

#[test]
fn peek_for_lua_colon_method_is_method_kind() {
    let output = peek(&["-k", "method", "cube", "tests/fixtures/lua"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].kind, "method");
    assert_eq!(results[0].scope, "math_utils.cube");
}

#[test]
fn peek_for_lua_dot_method_is_function_kind() {
    let output = peek(&["-k", "function", "square", "tests/fixtures/lua"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "math_utils.square" && r.kind == "function")
    );
}

#[test]
fn peek_for_lua_method_kind_excludes_dot_functions() {
    let output = peek(&["-k", "method", "square", "tests/fixtures/lua"]);
    assert_eq!(
        output.status.code(),
        Some(1),
        "-k method should not match dot-access functions"
    );
}

#[test]
fn peek_for_lua_no_false_positive() {
    // local variables should never be extracted
    let output = peek(&["x", "tests/fixtures/lua"]);
    assert_eq!(
        output.status.code(),
        Some(1),
        "expected no match for local variable"
    );
}

// === Lua top-level kind+scope verification tests ===

#[test]
fn peek_for_lua_global_function() {
    let output = peek(&["-k", "function", "initialize", "tests/fixtures/lua"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "initialize" && r.kind == "function")
    );
}

#[test]
fn peek_for_lua_local_function() {
    let output = peek(&["-k", "function", "validate", "tests/fixtures/lua"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "validate" && r.kind == "function")
    );
}

#[test]
fn peek_for_lua_function_signature_with_params() {
    let output = peek(&["-k", "function", "add", "tests/fixtures/lua"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    let def = results
        .iter()
        .find(|r| r.scope == "add" && r.kind == "function")
        .expect("expected 'add' function definition");
    assert!(
        def.signature.contains("a, b"),
        "expected signature to contain 'a, b', got: {}",
        def.signature
    );
}
