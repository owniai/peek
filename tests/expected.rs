mod common;

use common::{compare_expected, format_diff, load_expected, parse_json_matches, peek};

fn run_expected_test(lang: &str) {
    let expected_path = format!("tests/expected/{lang}.jsonl");
    let fixture_dir = format!("tests/fixtures/{lang}/");

    let expected = load_expected(&expected_path);
    let output = peek(&["outline", &fixture_dir, "--json"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let actual = parse_json_matches(&stdout);

    let report = compare_expected(expected, actual);
    let diff = format_diff(lang, &report);

    assert!(!report.has_hard_failures(), "{diff}");
}

#[test]
fn expected_python() {
    run_expected_test("python");
}

#[test]
fn expected_go() {
    run_expected_test("go");
}

#[test]
fn expected_rust() {
    run_expected_test("rust");
}

#[test]
fn expected_javascript() {
    run_expected_test("javascript");
}

#[test]
fn expected_typescript() {
    run_expected_test("typescript");
}

#[test]
fn expected_c() {
    run_expected_test("c");
}

#[test]
fn expected_cpp() {
    run_expected_test("cpp");
}

#[test]
fn expected_csharp() {
    run_expected_test("csharp");
}

#[test]
fn expected_java() {
    run_expected_test("java");
}

#[test]
fn expected_php() {
    run_expected_test("php");
}

#[test]
fn expected_kotlin() {
    run_expected_test("kotlin");
}

#[test]
fn expected_swift() {
    run_expected_test("swift");
}

#[test]
fn expected_ruby() {
    run_expected_test("ruby");
}

#[test]
fn expected_dart() {
    run_expected_test("dart");
}

#[test]
fn expected_bash() {
    run_expected_test("bash");
}

#[test]
fn expected_lua() {
    run_expected_test("lua");
}

#[test]
fn expected_luau() {
    run_expected_test("luau");
}

#[test]
fn expected_objc() {
    run_expected_test("objc");
}
