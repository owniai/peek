mod common;

use common::{parse_defs, peek};

// === C++ integration tests ===

#[test]
fn peek_for_cpp_namespace_function_scope() {
    let output = peek(&["-k", "function", "run", "tests/fixtures/cpp"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "Core::run" && r.kind == "function")
    );
}

#[test]
fn peek_for_cpp_namespace_class_scope() {
    let output = peek(&["-k", "class", "Service", "tests/fixtures/cpp"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "Core::Service" && r.kind == "class")
    );
}

#[test]
fn peek_for_cpp_namespace_struct_scope() {
    let output = peek(&["-k", "struct", "Config", "tests/fixtures/cpp"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "Core::Config" && r.kind == "struct")
    );
}

#[test]
fn peek_for_cpp_namespace_enum_scope() {
    let output = peek(&["-k", "enum", "Status", "tests/fixtures/cpp"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "Core::Status" && r.kind == "enum")
    );
}

#[test]
fn peek_for_cpp_namespace_const_scope() {
    let output = peek(&["-k", "const", "TIMEOUT", "tests/fixtures/cpp"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "Core::TIMEOUT" && r.kind == "const")
    );
}

#[test]
fn peek_for_cpp_namespace_type_scope() {
    let output = peek(&["-k", "alias", "Processor", "tests/fixtures/cpp"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "Core::Processor" && r.kind == "alias")
    );
}

#[test]
fn peek_for_cpp_method_in_class_scope() {
    let output = peek(&["-k", "method", "execute", "tests/fixtures/cpp"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "Core::Service::execute" && r.kind == "method")
    );
}

#[test]
fn peek_for_cpp_nested_class_scope() {
    let output = peek(&["-k", "method", "validate", "tests/fixtures/cpp"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "Core::Container::Item::validate" && r.kind == "method")
    );
}

#[test]
fn peek_for_cpp_nested_namespace_function_scope() {
    let output = peek(&["-k", "function", "compute", "tests/fixtures/cpp"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "App::Detail::compute" && r.kind == "function")
    );
}

#[test]
fn peek_for_cpp_nested_namespace_const_scope() {
    let output = peek(&["-k", "const", "BUFFER_SIZE", "tests/fixtures/cpp"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "App::Detail::BUFFER_SIZE" && r.kind == "const")
    );
}

#[test]
fn peek_for_finds_cpp_enum_class() {
    let output = peek(&["-k", "enum", "Direction", "tests/fixtures/cpp"]);
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
fn peek_for_finds_cpp_using_alias() {
    let output = peek(&["-k", "alias", "Callback", "tests/fixtures/cpp"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(results.iter().any(|r| r.scope == "Callback"));
}

// --- Supplementary: top-level definition kind+scope validation ---

#[test]
fn peek_for_cpp_top_level_function_scope() {
    let output = peek(&["-k", "function", "process", "tests/fixtures/cpp"]);
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
fn peek_for_cpp_top_level_struct_scope() {
    let output = peek(&["-k", "struct", "Point", "tests/fixtures/cpp"]);
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
fn peek_for_cpp_top_level_typedef_scope() {
    let output = peek(&["-k", "alias", "StatusCode", "tests/fixtures/cpp"]);
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
fn peek_for_cpp_top_level_const_scope() {
    let output = peek(&["-k", "const", "MAX_SIZE", "tests/fixtures/cpp"]);
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
fn peek_for_cpp_top_level_constexpr_scope() {
    let output = peek(&["-k", "const", "MAX_THREADS", "tests/fixtures/cpp"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "MAX_THREADS" && r.kind == "const")
    );
}

#[test]
fn peek_for_cpp_top_level_class_scope() {
    let output = peek(&["-k", "class", "Engine", "tests/fixtures/cpp"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    // Engine exists in both comprehensive.cpp and out_of_class_method.cpp
    assert!(
        results
            .iter()
            .any(|r| r.scope == "Engine" && r.kind == "class")
    );
}

#[test]
fn peek_for_cpp_top_level_enum_scope() {
    let output = peek(&["-k", "enum", "Color", "tests/fixtures/cpp"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "Color" && r.kind == "enum")
    );
}

// NOTE: No macro test — neither comprehensive.cpp nor out_of_class_method.cpp contains #define directives.

// === Field tests ===

#[test]
fn peek_for_cpp_struct_field() {
    let output = peek(&["-k", "field", "-w", "x", "tests/fixtures/cpp"]);
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
fn peek_for_cpp_class_field() {
    // Config struct in Core namespace has field timeout
    let output = peek(&["-k", "field", "timeout", "tests/fixtures/cpp"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.kind == "field" && r.scope == "Core::Config::timeout")
    );
}

#[test]
fn peek_for_cpp_field_kind_excludes_struct() {
    let output = peek(&["-k", "struct", "-w", "x", "tests/fixtures/cpp"]);
    assert_eq!(
        output.status.code(),
        Some(1),
        "field name should not match -k struct"
    );
}

// === Static tests ===

#[test]
fn peek_for_cpp_file_scope_static() {
    let output = peek(&["-k", "static", "-w", "file_count", "tests/fixtures/cpp"]);
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
fn peek_for_cpp_namespace_static() {
    let output = peek(&["-k", "static", "-w", "counter", "tests/fixtures/cpp"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(!results.is_empty());
    assert!(
        results
            .iter()
            .any(|r| r.kind == "static" && r.scope == "Core::counter")
    );
}

#[test]
fn peek_for_cpp_static_const_is_const_not_static() {
    // VERSION is static const int -- should be Const, not Static
    let output = peek(&["-k", "static", "-w", "TIMEOUT", "tests/fixtures/cpp"]);
    assert_eq!(
        output.status.code(),
        Some(1),
        "static const should be Const, not Static"
    );
}

#[test]
fn peek_for_cpp_static_pointer_var() {
    let output = peek(&["-k", "static", "-w", "file_name", "tests/fixtures/cpp"]);
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

// === Value category expansion ===

#[test]
fn peek_for_cpp_value_category_includes_field_and_static() {
    let output = peek(&["-k", "value", "-w", "file_count", "tests/fixtures/cpp"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(!results.is_empty());
    assert!(results.iter().any(|r| r.kind == "static"));
}

// === Namespace tests ===

#[test]
fn peek_for_cpp_namespace_kind_core() {
    let output = peek(&["-k", "namespace", "-w", "Core", "tests/fixtures/cpp"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "Core" && r.kind == "namespace"),
        "expected namespace Core, got: {results:?}"
    );
}

#[test]
fn peek_for_cpp_namespace_kind_nested() {
    let output = peek(&["-k", "namespace", "-w", "App::Detail", "tests/fixtures/cpp"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "App::Detail" && r.kind == "namespace"),
        "expected namespace App::Detail, got: {results:?}"
    );
}
