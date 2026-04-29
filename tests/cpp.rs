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
    let output = peek(&["-k", "type", "Processor", "tests/fixtures/cpp"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "Core::Processor" && r.kind == "type")
    );
}

#[test]
fn peek_for_cpp_method_in_class_scope() {
    let output = peek(&["-k", "function", "execute", "tests/fixtures/cpp"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "Core::Service::execute" && r.kind == "function")
    );
}

#[test]
fn peek_for_cpp_nested_class_scope() {
    let output = peek(&["-k", "function", "validate", "tests/fixtures/cpp"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "Core::Container::Item::validate" && r.kind == "function")
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
    let output = peek(&["-k", "type", "Callback", "tests/fixtures/cpp"]);
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
    let output = peek(&["-k", "type", "StatusCode", "tests/fixtures/cpp"]);
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
