mod common;

use common::{parse_defs, peek};

// === Kotlin scope verification & edge-case tests ===

#[test]
fn peek_for_kotlin_nested_class_scope() {
    let output = peek(&["-k", "class", "Inner", "tests/fixtures/kotlin"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "Outer.Inner" && r.kind == "class")
    );
}

#[test]
fn peek_for_kotlin_nested_interface_scope() {
    let output = peek(&["-k", "interface", "Handler", "tests/fixtures/kotlin"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "Outer.Handler" && r.kind == "interface")
    );
}

#[test]
fn peek_for_kotlin_nested_object_scope() {
    let output = peek(&["-k", "object", "Cache", "tests/fixtures/kotlin"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "Outer.Cache" && r.kind == "object")
    );
}

#[test]
fn peek_for_kotlin_nested_enum_scope() {
    let output = peek(&["-k", "enum", "Priority", "tests/fixtures/kotlin"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "Outer.Priority" && r.kind == "enum")
    );
}

#[test]
fn peek_for_kotlin_deeply_nested_scope() {
    let output = peek(&["-k", "class", "Config", "tests/fixtures/kotlin"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    // Container.Builder.Config
    assert!(
        results
            .iter()
            .any(|r| r.scope == "Container.Builder.Config" && r.kind == "class")
    );
}

#[test]
fn peek_for_kotlin_method_scope() {
    let output = peek(&["-k", "function", "greet", "tests/fixtures/kotlin"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "SimpleClass.greet" && r.kind == "function")
    );
}

#[test]
fn peek_for_kotlin_sealed_class_nested_scope() {
    let output = peek(&["-k", "class", "Circle", "tests/fixtures/kotlin"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "Shape.Circle" && r.kind == "class")
    );
}

#[test]
fn peek_for_kotlin_sealed_interface_nested_scope() {
    let output = peek(&["-k", "class", "Literal", "tests/fixtures/kotlin"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "Node.Literal" && r.kind == "class")
    );
}

#[test]
fn peek_for_kotlin_object_const_scope() {
    let output = peek(&["-k", "const", "PI", "tests/fixtures/kotlin"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "MathUtils.PI" && r.kind == "const")
    );
}

#[test]
fn peek_for_kotlin_companion_object_const() {
    let output = peek(&["-k", "const", "DEFAULT_TIMEOUT", "tests/fixtures/kotlin"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "Config.Defaults.DEFAULT_TIMEOUT" && r.kind == "const")
    );
}

#[test]
fn peek_for_kotlin_multiple_consts() {
    let output = peek(&["-k", "const", "APP_VERSION", "tests/fixtures/kotlin"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert!(
        results
            .iter()
            .any(|r| r.scope == "APP_VERSION" && r.kind == "const")
    );
    let output2 = peek(&["-k", "const", "MAX_RETRIES", "tests/fixtures/kotlin"]);
    let stdout2 = String::from_utf8_lossy(&output2.stdout);
    assert!(output2.status.success());
    let results2 = parse_defs(&stdout2);
    assert!(
        results2
            .iter()
            .any(|r| r.scope == "MAX_RETRIES" && r.kind == "const")
    );
}

#[test]
fn peek_for_kotlin_no_false_positive() {
    // package and import declarations should never be extracted
    let output = peek(&["com", "tests/fixtures/kotlin"]);
    assert_eq!(
        output.status.code(),
        Some(1),
        "expected no match for package/import"
    );
}

// --- Kotlin extension function ---

#[test]
fn peek_for_kotlin_extension_function_found() {
    let output = peek(&["-k", "function", "isEmail", "tests/fixtures/kotlin"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    // BUG: Extension function `fun String.isEmail()` should be found but is not,
    // because the regex `fun\s+(?:isEmail)\b` cannot match `fun String.isEmail()`.
    let results = parse_defs(&stdout);
    assert!(
        !results.is_empty(),
        "BUG: extension function 'isEmail' not found. Got: {}",
        stdout
    );
}

#[test]
fn peek_for_kotlin_extension_function_with_modifier_found() {
    let output = peek(&["-k", "function", "isPositive", "tests/fixtures/kotlin"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    // BUG: `private fun Int.isPositive()` should be found but is not.
    let results = parse_defs(&stdout);
    assert!(
        !results.is_empty(),
        "BUG: extension function 'isPositive' not found. Got: {}",
        stdout
    );
}

#[test]
fn peek_for_kotlin_regular_function_still_works() {
    // Verify regular (non-extension) functions are still found correctly
    let output = peek(&["-k", "function", "regularFunction", "tests/fixtures/kotlin"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert_eq!(results.len(), 1);
    assert!(stdout.contains("regularFunction"));
}
