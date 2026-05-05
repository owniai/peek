mod common;

use common::{parse_defs, peek};

// === PHP integration tests ===

#[test]
fn peek_for_php_class_scope_simple_ns() {
    let output = peek(&["-k", "class", "User", "tests/fixtures/php"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "App\\Models\\User" && r.kind == "class")
    );
}

#[test]
fn peek_for_php_class_scope_brace_ns() {
    let output = peek(&["-k", "class", "UserService", "tests/fixtures/php"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "App\\Services\\UserService" && r.kind == "class")
    );
}

#[test]
fn peek_for_php_method_scope() {
    let output = peek(&["-k", "method", "getName", "tests/fixtures/php"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "App\\Models\\User::getName" && r.kind == "method")
    );
}

#[test]
fn peek_for_php_const_scope() {
    let output = peek(&["-k", "const", "MIN_AGE", "tests/fixtures/php"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "App\\Models\\User::MIN_AGE" && r.kind == "const")
    );
}

#[test]
fn peek_for_php_brace_namespace_function() {
    let output = peek(&["-k", "function", "global_func", "tests/fixtures/php"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "global_func" && r.kind == "function")
    );
}

#[test]
fn peek_for_php_brace_namespace_const() {
    let output = peek(&["-k", "const", "GLOBAL_CONST", "tests/fixtures/php"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "GLOBAL_CONST" && r.kind == "const")
    );
}

#[test]
fn peek_for_php_multi_const() {
    // Multi-constant declarations (e.g., "const DEBUG = true, CACHE_TTL = 3600, MAX_ITEMS = 100;")
    // extract all names in the declaration via AST parsing.

    // Clean cache for deterministic first-search behavior
    let _ = std::fs::remove_dir_all(".peek-cache");

    let output = peek(&["-k", "const", "DEBUG", "tests/fixtures/php"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope.contains("DEBUG") && r.kind == "const")
    );
}

#[test]
fn peek_for_php_mixed_html_class() {
    let output = peek(&["-k", "class", "Page", "tests/fixtures/php"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    assert!(stdout.contains("[class/"));
    assert!(stdout.contains("Page"));
}

#[test]
fn peek_for_php_mixed_html_method() {
    let output = peek(&["-k", "method", "render", "tests/fixtures/php"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    // render() is in mixed_html.php Page class and comprehensive.php Renderable interface
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope.contains("Page") && r.signature.contains("render"))
    );
}

#[test]
fn peek_for_php_attribute_class() {
    let output = peek(&["-k", "class", "UserController", "tests/fixtures/php"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "App\\Attributes\\UserController" && r.kind == "class")
    );
}

#[test]
fn peek_for_php_no_false_positive() {
    // echo statements, use declarations, namespace keywords should never be extracted
    let output = peek(&["echo", "tests/fixtures/php"]);
    assert_eq!(
        output.status.code(),
        Some(1),
        "expected no match for echo statement"
    );
}

#[test]
fn peek_for_php_trait_scope() {
    let output = peek(&["-k", "trait", "Loggable", "tests/fixtures/php"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "App\\Models\\Loggable" && r.kind == "trait")
    );
}

#[test]
fn peek_for_php_trait_method_scope() {
    let output = peek(&["-k", "method", "log", "tests/fixtures/php"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "App\\Models\\Loggable::log" && r.kind == "method")
    );
}

#[test]
fn peek_for_php_typed_const() {
    // PHP 8.3+ typed constants (e.g., "const string APP_NAME = 'peek'")
    // are found via AST parsing.
    let output = peek(&[
        "-k",
        "const",
        "APP_NAME",
        "tests/fixtures/php/typed_const.php",
    ]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    assert!(
        stdout.contains("[const/"),
        "PHP 8.3+ typed constant 'APP_NAME' not found. Got: {}",
        stdout
    );
}

// --- Supplementary: top-level definition kind+scope validation ---

#[test]
fn peek_for_php_top_level_class_scope() {
    // Page class in mixed_html.php has no namespace declaration
    let output = peek(&["-k", "class", "Page", "tests/fixtures/php/mixed_html.php"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "Page" && r.kind == "class")
    );
}

#[test]
fn peek_for_php_top_level_function_scope() {
    // helper() in namespace_brace.php is inside `namespace {}` (global namespace), scope is just "helper"
    let output = peek(&[
        "-k",
        "function",
        "helper",
        "tests/fixtures/php/namespace_brace.php",
    ]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "helper" && r.kind == "function")
    );
}

#[test]
fn peek_for_php_enum_scope() {
    let output = peek(&["-k", "enum", "Status", "tests/fixtures/php"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "App\\Models\\Status" && r.kind == "enum")
    );
}

#[test]
fn peek_for_php_interface_scope() {
    let output = peek(&["-k", "interface", "Renderable", "tests/fixtures/php"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "App\\Models\\Renderable" && r.kind == "interface")
    );
}

// === PHP Property tests ===

#[test]
fn peek_for_php_typed_property() {
    // public string $name in User class
    let output = peek(&["-k", "property", "name", "tests/fixtures/php"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "App\\Models\\User::name" && r.kind == "property"),
        "expected property User::name, got: {results:?}"
    );
}

#[test]
fn peek_for_php_int_property() {
    // public int $age in User class
    let output = peek(&["-k", "property", "age", "tests/fixtures/php"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "App\\Models\\User::age" && r.kind == "property"),
        "expected property User::age, got: {results:?}"
    );
}

#[test]
fn peek_for_php_property_not_const() {
    // $name should NOT appear as const
    let output = peek(&["-k", "const", "name", "tests/fixtures/php"]);
    assert_eq!(
        output.status.code(),
        Some(1),
        "property should not match -k const"
    );
}

#[test]
fn peek_for_php_value_category_includes_property() {
    let output = peek(&["-k", "value", "name", "tests/fixtures/php"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results.iter().any(|r| r.scope == "App\\Models\\User::name"),
        "expected User::name in value category, got: {results:?}"
    );
}

// === Namespace tests ===

#[test]
fn peek_for_php_namespace_kind_simple() {
    // Use -e with properly escaped backslash for regex pattern matching PHP namespace scope
    let output = peek(&[
        "-k",
        "namespace",
        "-e",
        "App\\\\Models",
        "tests/fixtures/php",
    ]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "App\\Models" && r.kind == "namespace"),
        "expected namespace App\\Models, got: {results:?}"
    );
}

#[test]
fn peek_for_php_namespace_kind_brace() {
    let output = peek(&[
        "-k",
        "namespace",
        "-e",
        "App\\\\Services",
        "tests/fixtures/php",
    ]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "App\\Services" && r.kind == "namespace"),
        "expected namespace App\\Services, got: {results:?}"
    );
}

#[test]
fn peek_for_php_namespace_kind_attribute() {
    let output = peek(&[
        "-k",
        "namespace",
        "-e",
        "App\\\\Attributes",
        "tests/fixtures/php",
    ]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "App\\Attributes" && r.kind == "namespace"),
        "expected namespace App\\Attributes, got: {results:?}"
    );
}
