mod common;

use common::{parse_defs, peek};

// === Scope tests ===

#[test]
fn peek_for_rust_impl_method_scope() {
    let output = peek(&["-k", "function", "new", "tests/fixtures/rust"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert_eq!(results.len(), 3);
    let scopes: Vec<&str> = results.iter().map(|r| r.scope.as_str()).collect();
    assert!(
        scopes.contains(&"MyStruct::new"),
        "expected 'MyStruct::new', got: {scopes:?}"
    );
    assert!(
        scopes.contains(&"GenericStruct::new"),
        "expected 'GenericStruct::new', got: {scopes:?}"
    );
    assert!(
        scopes.contains(&"container::Container::new"),
        "expected 'container::Container::new', got: {scopes:?}"
    );
}

#[test]
fn peek_for_rust_mod_nested_scope() {
    let output = peek(&["-w", "-k", "function", "mod_func", "tests/fixtures/rust"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].scope, "my_module::mod_func");
}

#[test]
fn peek_for_rust_deeply_nested_scope() {
    let output = peek(&["-k", "function", "deep_func", "tests/fixtures/rust"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].scope, "my_module::nested::deep_func");
}

#[test]
fn peek_for_rust_impl_method_in_mod_scope() {
    let output = peek(&["-w", "-k", "function", "method", "tests/fixtures/rust"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].scope, "my_module::ModStruct::method");
}

#[test]
fn peek_for_rust_trait_method_scope() {
    let output = peek(&["-k", "function", "required_method", "tests/fixtures/rust"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].scope, "MyTrait::required_method");
}

#[test]
fn peek_for_rust_trait_impl_method_scope() {
    let output = peek(&["-k", "function", "fmt", "tests/fixtures/rust"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].scope, "MyStruct::fmt");
}

#[test]
fn peek_for_rust_struct_scope() {
    let output = peek(&["-k", "struct", "DeriveStruct", "tests/fixtures/rust"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].kind, "struct");
    assert_eq!(results[0].scope, "DeriveStruct");
    assert!(results[0].signature.contains("#[derive(Debug, Clone)]"));
}

// === Edge case tests ===

#[test]
fn peek_for_rust_async_method_signature() {
    let output = peek(&["-k", "function", "async_method", "tests/fixtures/rust"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].scope, "MyStruct::async_method");
    assert!(results[0].signature.contains("async"));
}

#[test]
fn peek_for_rust_attribute_in_signature() {
    let output = peek(&["-k", "function", "inline_func", "tests/fixtures/rust"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert_eq!(results.len(), 1);
    assert!(
        results[0].signature.contains("#[inline]"),
        "signature should include attribute, got: {}",
        results[0].signature
    );
}

// === False positive tests ===

#[test]
fn peek_for_rust_no_false_positive_import() {
    let output = peek(&["std", "tests/fixtures/rust"]);
    assert_eq!(
        output.status.code(),
        Some(1),
        "use/import statements should not be matched"
    );
}
