mod common;

use common::{parse_defs, peek};

// === From integration_test.rs: basic Python tests ===

#[test]
fn peek_for_nested_class_scope() {
    let output = peek(&["-k", "class", "InnerClass", "tests/fixtures/python"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    assert!(stdout.contains("MyClass.InnerClass"));
}

#[test]
fn peek_output_has_expected_format() {
    let output = peek(&["top_level_func", "tests/fixtures/python"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let results = parse_defs(&stdout);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].kind, "function");
    assert_eq!(results[0].scope, "top_level_func");
}

// === From python_batch1.rs: basic functions and classes ===

#[test]
fn test_python_basic_functions() {
    let path = "tests/fixtures/python/basic_functions.py";

    // simple_func: 1 result, kind=function, top-level (no scope)
    let output = peek(&["-k", "function", "simple_func", path]);
    assert!(output.status.success());
    let results = parse_defs(&String::from_utf8_lossy(&output.stdout));
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].kind, "function");
    assert_eq!(results[0].scope, "simple_func");

    // typed_func: signature contains "x: int" and "-> str"
    let output = peek(&["-k", "function", "typed_func", path]);
    assert!(output.status.success());
    let results = parse_defs(&String::from_utf8_lossy(&output.stdout));
    assert_eq!(results.len(), 1);
    assert!(
        results[0].signature.contains("x: int"),
        "signature should contain 'x: int', got: {}",
        results[0].signature
    );
    assert!(
        results[0].signature.contains("-> str"),
        "signature should contain '-> str', got: {}",
        results[0].signature
    );

    // _private_helper: 1 result (private functions not filtered), top-level
    let output = peek(&["-k", "function", "_private_helper", path]);
    assert!(output.status.success());
    let results = parse_defs(&String::from_utf8_lossy(&output.stdout));
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].scope, "_private_helper");

    // __dunder_special__: 1 result (dunder functions not filtered), top-level
    let output = peek(&["-k", "function", "__dunder_special__", path]);
    assert!(output.status.success());
    let results = parse_defs(&String::from_utf8_lossy(&output.stdout));
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].scope, "__dunder_special__");

    // default_params: 1 result, top-level
    let output = peek(&["-k", "function", "default_params", path]);
    assert!(output.status.success());
    let results = parse_defs(&String::from_utf8_lossy(&output.stdout));
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].scope, "default_params");
}

#[test]
fn test_python_basic_classes() {
    let path = "tests/fixtures/python/basic_classes.py";

    // SimpleClass: 1 result, kind=class, top-level
    let output = peek(&["-k", "class", "SimpleClass", path]);
    assert!(output.status.success());
    let results = parse_defs(&String::from_utf8_lossy(&output.stdout));
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].kind, "class");
    assert_eq!(results[0].scope, "SimpleClass");

    // instance_method: 1 result, scope=SimpleClass.instance_method
    let output = peek(&["-k", "function", "instance_method", path]);
    assert!(output.status.success());
    let results = parse_defs(&String::from_utf8_lossy(&output.stdout));
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].scope, "SimpleClass.instance_method");

    // EmptyClass: 1 result, single-line class (start == end)
    let output = peek(&["-k", "class", "EmptyClass", path]);
    assert!(output.status.success());
    let results = parse_defs(&String::from_utf8_lossy(&output.stdout));
    assert_eq!(results.len(), 1);
    assert_eq!(
        results[0].start, results[0].end,
        "EmptyClass should be a single-line class"
    );

    // MultiInherit: 1 result, top-level
    let output = peek(&["-k", "class", "MultiInherit", path]);
    assert!(output.status.success());
    let results = parse_defs(&String::from_utf8_lossy(&output.stdout));
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].scope, "MultiInherit");
}

#[test]
fn test_python_class_methods_scope() {
    let path = "tests/fixtures/python/basic_classes.py";

    // static_helper: scope contains class name (StaticHolder.static_helper)
    let output = peek(&["-k", "function", "static_helper", path]);
    assert!(output.status.success());
    let results = parse_defs(&String::from_utf8_lossy(&output.stdout));
    assert_eq!(results.len(), 1);
    let scope = &results[0].scope;
    assert!(
        scope.contains("StaticHolder"),
        "scope should contain 'StaticHolder', got: {scope}"
    );

    // factory_method: scope contains class name (ClassMeta.factory_method)
    let output = peek(&["-k", "function", "factory_method", path]);
    assert!(output.status.success());
    let results = parse_defs(&String::from_utf8_lossy(&output.stdout));
    assert_eq!(results.len(), 1);
    let scope = &results[0].scope;
    assert!(
        scope.contains("ClassMeta"),
        "scope should contain 'ClassMeta', got: {scope}"
    );

    // __init__: scope contains class name (InitializerClass.__init__)
    let output = peek(&["-k", "function", "__init__", path]);
    assert!(output.status.success());
    let results = parse_defs(&String::from_utf8_lossy(&output.stdout));
    assert_eq!(results.len(), 1);
    let scope = &results[0].scope;
    assert!(
        scope.contains("InitializerClass"),
        "scope should contain 'InitializerClass', got: {scope}"
    );
}

#[test]
fn test_python_async_definitions() {
    let path = "tests/fixtures/python/async_definitions.py";

    // async_fetch: 1 result, kind=function (not async_function)
    let output = peek(&["-k", "function", "async_fetch", path]);
    assert!(output.status.success());
    let results = parse_defs(&String::from_utf8_lossy(&output.stdout));
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].kind, "function");

    // process: 1 result, scope=AsyncService.process
    let output = peek(&["-k", "function", "process", path]);
    assert!(output.status.success());
    let results = parse_defs(&String::from_utf8_lossy(&output.stdout));
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].scope, "AsyncService.process");

    // async_static: 1 result, scope=AsyncHelper.async_static
    let output = peek(&["-k", "function", "async_static", path]);
    assert!(output.status.success());
    let results = parse_defs(&String::from_utf8_lossy(&output.stdout));
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].scope, "AsyncHelper.async_static");

    // create: 1 result, scope=AsyncFactory.create
    let output = peek(&["-k", "function", "create", path]);
    assert!(output.status.success());
    let results = parse_defs(&String::from_utf8_lossy(&output.stdout));
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].scope, "AsyncFactory.create");
}

// === From python_batch2.rs: modern syntax, tricky code, large scale ===

#[test]
fn test_python_modern_syntax() {
    let path = "tests/fixtures/python/modern_syntax.py";

    // dataclass
    let output = peek(&["-k", "class", "UserRecord", path]);
    assert!(output.status.success());
    let results = parse_defs(&String::from_utf8_lossy(&output.stdout));
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].kind, "class");
    assert_eq!(results[0].scope, "UserRecord");

    // Enum
    let output = peek(&["-k", "class", "Color", path]);
    assert!(output.status.success());
    let results = parse_defs(&String::from_utf8_lossy(&output.stdout));
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].kind, "class");
    assert_eq!(results[0].scope, "Color");

    // Protocol
    let output = peek(&["-k", "class", "Serializable", path]);
    assert!(output.status.success());
    let results = parse_defs(&String::from_utf8_lossy(&output.stdout));
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].kind, "class");
    assert_eq!(results[0].scope, "Serializable");

    // Complex type annotations should not prevent recognition
    let output = peek(&["-k", "function", "complex_types", path]);
    assert!(output.status.success());
    let results = parse_defs(&String::from_utf8_lossy(&output.stdout));
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].kind, "function");
    assert_eq!(results[0].scope, "complex_types");

    // Generic class
    let output = peek(&["-k", "class", "Container", path]);
    assert!(output.status.success());
    let results = parse_defs(&String::from_utf8_lossy(&output.stdout));
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].kind, "class");
    assert_eq!(results[0].scope, "Container");

    // TypedDict
    let output = peek(&["-k", "class", "Point", path]);
    assert!(output.status.success());
    let results = parse_defs(&String::from_utf8_lossy(&output.stdout));
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].kind, "class");
    assert_eq!(results[0].scope, "Point");
}

#[test]
fn test_python_tricky_no_false_positives() {
    let path = "tests/fixtures/python/tricky_code.py";

    // Real definition exists
    let output = peek(&["-k", "function", "real_func", path]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert_eq!(results.len(), 1);

    // String-embedded fake definition
    let output = peek(&["-k", "function", "fake_in_string", path]);
    assert_eq!(
        output.status.code(),
        Some(1),
        "expected no match for string-embedded fake"
    );

    // Comment-embedded fake class
    let output = peek(&["-k", "class", "FakeInComment", path]);
    assert_eq!(
        output.status.code(),
        Some(1),
        "expected no match for comment-embedded fake"
    );

    // Triple-quoted string fake function
    let output = peek(&["-k", "function", "fake_in_triple_quotes", path]);
    assert_eq!(
        output.status.code(),
        Some(1),
        "expected no match for triple-quoted string fake"
    );

    // Triple-quoted string fake class
    let output = peek(&["-k", "class", "AlsoFake", path]);
    assert_eq!(
        output.status.code(),
        Some(1),
        "expected no match for triple-quoted string fake class"
    );
}

#[test]
fn test_python_tricky_multiline_sig() {
    let path = "tests/fixtures/python/tricky_code.py";

    // multiline_params: start line should be the def line (line 15)
    let output = peek(&["-k", "function", "multiline_params", path]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].start, 15);
    assert!(
        results[0].signature.contains("def multiline_params"),
        "signature should contain 'def multiline_params', got: {}",
        results[0].signature
    );

    // MinimalClass: single-line definition, start == end
    let output = peek(&["-k", "class", "MinimalClass", path]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].start, results[0].end);
}

#[test]
fn test_python_large_scale_count() {
    let path = "tests/fixtures/python/large_scale.py";

    // Verify same-name definitions return 3 results for "initialize"
    let output = peek(&["initialize", path]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert_eq!(results.len(), 3);

    // func initialize -> 3 results
    let output = peek(&["-k", "function", "initialize", path]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert_eq!(results.len(), 3);

    // func transform -> 2 results (use -w for whole-word matching to exclude typed_transform etc.)
    let output = peek(&["-w", "-k", "function", "transform", path]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert_eq!(results.len(), 2);

    // class Node -> 2 results
    let output = peek(&["-k", "class", "Node", path]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert_eq!(results.len(), 2);
}

#[test]
fn test_python_large_scale_scope_sampling() {
    let path = "tests/fixtures/python/large_scale.py";

    // initialize: 3 results with expected scopes
    let output = peek(&["-k", "function", "initialize", path]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert_eq!(results.len(), 3);
    let scopes: Vec<&str> = results.iter().map(|r| r.scope.as_str()).collect();
    assert!(
        scopes.contains(&"initialize"),
        "expected top-level scope 'initialize', got scopes: {scopes:?}"
    );
    assert!(
        scopes.contains(&"ServiceContainer.ServiceA.initialize"),
        "expected scope 'ServiceContainer.ServiceA.initialize', got scopes: {scopes:?}"
    );
    assert!(
        scopes.contains(&"ServiceContainer.ServiceB.initialize"),
        "expected scope 'ServiceContainer.ServiceB.initialize', got scopes: {scopes:?}"
    );

    // class Node: 2 results with Tree.Node and Graph.Node
    let output = peek(&["-k", "class", "Node", path]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert_eq!(results.len(), 2);
    let scopes: Vec<&str> = results.iter().map(|r| r.scope.as_str()).collect();
    assert!(
        scopes.contains(&"Tree.Node"),
        "expected scope 'Tree.Node', got scopes: {scopes:?}"
    );
    assert!(
        scopes.contains(&"Graph.Node"),
        "expected scope 'Graph.Node', got scopes: {scopes:?}"
    );

    // Verify all definition lines match expected format
    let all_lines: Vec<&str> = stdout.lines().collect();
    for line in all_lines.iter().skip(1) {
        assert!(
            line.contains("[class/") || line.contains("[function/") || line.contains("[struct/"),
            "expected valid format, got: {line}"
        );
    }
}

// === From python_batch3.rs: decorators, nested classes, scope resolution ===

#[test]
fn test_python_decorated_functions() {
    // retried_func: single decorator
    let output = peek(&[
        "-k",
        "function",
        "retried_func",
        "tests/fixtures/python/decorators.py",
    ]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].kind, "function");
    assert_eq!(results[0].scope, "retried_func");
    assert!(
        results[0].signature.starts_with("@retry def retried_func"),
        "signature should include decorator, got: {}",
        results[0].signature
    );

    // stacked_func: stacked decorators
    let output = peek(&[
        "-k",
        "function",
        "stacked_func",
        "tests/fixtures/python/decorators.py",
    ]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].kind, "function");

    // api_handler: decorator with arguments
    let output = peek(&[
        "-k",
        "function",
        "api_handler",
        "tests/fixtures/python/decorators.py",
    ]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].kind, "function");
}

#[test]
fn test_python_decorated_classes() {
    // SingletonClass: decorated class
    let output = peek(&[
        "-k",
        "class",
        "SingletonClass",
        "tests/fixtures/python/decorators.py",
    ]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].kind, "class");

    // home: decorated method inside class
    let output = peek(&[
        "-k",
        "function",
        "home",
        "tests/fixtures/python/decorators.py",
    ]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].scope, "WebApp.home");

    // cached_data: stacked decorator method inside class
    let output = peek(&[
        "-k",
        "function",
        "cached_data",
        "tests/fixtures/python/decorators.py",
    ]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].scope, "WebApp.cached_data");
}

#[test]
fn test_python_nested_classes() {
    // InnerA: 2-layer nested class
    let output = peek(&[
        "-k",
        "class",
        "InnerA",
        "tests/fixtures/python/nested_definitions.py",
    ]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].scope, "OuterA.InnerA");

    // Level5: 5-layer deep nested class
    let output = peek(&[
        "-k",
        "class",
        "Level5",
        "tests/fixtures/python/nested_definitions.py",
    ]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].scope, "Level1.Level2.Level3.Level4.Level5");

    // deep_method: method inside 5-layer nested class
    let output = peek(&[
        "-k",
        "function",
        "deep_method",
        "tests/fixtures/python/nested_definitions.py",
    ]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert_eq!(results.len(), 1);
    assert_eq!(
        results[0].scope,
        "Level1.Level2.Level3.Level4.Level5.deep_method"
    );

    // inner_method: method inside 2-layer nested class
    let output = peek(&[
        "-k",
        "function",
        "inner_method",
        "tests/fixtures/python/nested_definitions.py",
    ]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].scope, "OuterB.InnerB.inner_method");

    // PartA: sibling nested class
    let output = peek(&[
        "-k",
        "class",
        "PartA",
        "tests/fixtures/python/nested_definitions.py",
    ]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].scope, "Container.PartA");

    // PartB: sibling nested class
    let output = peek(&[
        "-k",
        "class",
        "PartB",
        "tests/fixtures/python/nested_definitions.py",
    ]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].scope, "Container.PartB");
}

#[test]
fn test_python_same_name_different_scope() {
    // process: 3 results across different scopes
    let output = peek(&[
        "-k",
        "function",
        "process",
        "tests/fixtures/python/scope_resolution.py",
    ]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert_eq!(results.len(), 3);
    let scopes: Vec<&str> = results.iter().map(|r| r.scope.as_str()).collect();
    assert!(
        scopes.contains(&"process"),
        "expected top-level 'process', got: {scopes:?}"
    );
    assert!(
        scopes.contains(&"Alpha.process"),
        "expected 'Alpha.process' scope, got: {scopes:?}"
    );
    assert!(
        scopes.contains(&"Beta.process"),
        "expected 'Beta.process' scope, got: {scopes:?}"
    );

    // validate: 2 results
    let output = peek(&[
        "-k",
        "function",
        "validate",
        "tests/fixtures/python/scope_resolution.py",
    ]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert_eq!(results.len(), 2);
    let scopes: Vec<&str> = results.iter().map(|r| r.scope.as_str()).collect();
    assert!(
        scopes.contains(&"validate"),
        "expected top-level 'validate', got: {scopes:?}"
    );
    assert!(
        scopes.contains(&"Gamma.validate"),
        "expected 'Gamma.validate' scope, got: {scopes:?}"
    );

    // Item: 2 nested classes with same name
    let output = peek(&[
        "-k",
        "class",
        "Item",
        "tests/fixtures/python/scope_resolution.py",
    ]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert_eq!(results.len(), 2);
    let scopes: Vec<&str> = results.iter().map(|r| r.scope.as_str()).collect();
    assert!(
        scopes.contains(&"First.Item"),
        "expected 'First.Item' scope, got: {scopes:?}"
    );
    assert!(
        scopes.contains(&"Second.Item"),
        "expected 'Second.Item' scope, got: {scopes:?}"
    );
}

// === From python_signature_comments.rs: inline comment bug ===

/// Bug: inline comments in multi-line signatures cause truncation.
///
/// `strip_trailing_comment` is designed for single lines, but `flatten_bytes`
/// merges multiple lines into one. When the merged line contains a `#` comment
/// from an earlier line, `strip_trailing_comment` truncates everything after it,
/// including the actual function/class definition.
#[test]
fn test_python_multiline_params_with_comments() {
    let path = "tests/fixtures/python/signature_with_comments.py";

    // process_data: multiline params with trailing comments
    let output = peek(&["-k", "function", "process_data", path]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert_eq!(results.len(), 1);
    // BUG: signature should contain the full function signature including
    // all parameters and return type, but it gets truncated at the first `#`.
    // Expected: "def process_data(items: list, verbose: bool) -> None"
    // Actual:   "def process_data( items: list,"
    assert!(
        results[0].signature.contains("verbose"),
        "signature should contain 'verbose' parameter, got: {}",
        results[0].signature
    );
    assert!(
        results[0].signature.contains("-> None"),
        "signature should contain return type '-> None', got: {}",
        results[0].signature
    );
}

#[test]
fn test_python_decorated_func_with_comment() {
    let path = "tests/fixtures/python/signature_with_comments.py";

    // fetch_url: decorated function with comment on decorator line
    let output = peek(&["-k", "function", "fetch_url", path]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert_eq!(results.len(), 1);
    // BUG: signature is truncated to just "@retry(max_attempts=3)"
    // Expected: "@retry(max_attempts=3) def fetch_url(url: str) -> bytes"
    assert!(
        results[0].signature.contains("def fetch_url"),
        "signature should contain 'def fetch_url', got: {}",
        results[0].signature
    );
    assert!(
        results[0].signature.contains("url: str"),
        "signature should contain parameter 'url: str', got: {}",
        results[0].signature
    );
}

#[test]
fn test_python_stacked_decorators_with_comments() {
    let path = "tests/fixtures/python/signature_with_comments.py";

    // compute: stacked decorators with comments
    let output = peek(&["-k", "function", "compute", path]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert_eq!(results.len(), 1);
    // BUG: signature is truncated to just "@cache"
    // Expected: "@cache @timeout(30) def compute(x: int) -> int"
    assert!(
        results[0].signature.contains("def compute"),
        "signature should contain 'def compute', got: {}",
        results[0].signature
    );
}

#[test]
fn test_python_decorated_class_with_comment() {
    let path = "tests/fixtures/python/signature_with_comments.py";

    // UserRecord: decorated class with comment
    let output = peek(&["-k", "class", "UserRecord", path]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let results = parse_defs(&stdout);
    assert_eq!(results.len(), 1);
    // BUG: signature is truncated to just "@dataclass"
    // Expected: "@dataclass class UserRecord"
    assert!(
        results[0].signature.contains("class UserRecord"),
        "signature should contain 'class UserRecord', got: {}",
        results[0].signature
    );
}

// === From python_type_alias_signature.rs: multiline type alias bug ===

/// Bug: multiline type alias signature is not flattened to single line.
///
/// `type_alias_statement` signature uses `node.utf8_text()` directly instead of
/// `flatten_bytes()`, so multiline type aliases retain newlines in the signature.
/// This violates the project convention: "multi-line signatures are compressed
/// to a single line".
#[test]
fn test_python_multiline_type_alias_signature() {
    let path = "tests/fixtures/python/signature_with_comments.py";

    // Matrix: multiline type alias
    // The signature for a multiline type alias should be flattened to one line.
    // The raw stdout will have the signature spanning multiple lines, which is
    // the bug we're detecting.
    let output = peek(&["-k", "type", "Matrix", path]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());

    // Check the raw output: the signature line should contain the full type
    // on a single line. If the type alias definition line is followed by more
    // lines of the signature, that's the bug.
    // Expected output line: "...type/Matrix]: type Matrix = list[list[float],]"
    // Actual (buggy): "...type/Matrix]: type Matrix = list[" followed by more lines

    // The signature should include the closing bracket, meaning it was properly
    // flattened. With the bug, the first output line only contains "type Matrix = list["
    // and the rest of the signature spills to subsequent lines.
    let def_line = stdout
        .lines()
        .find(|l| l.contains("[type/Matrix]"))
        .expect("should find Matrix definition line");

    // If the signature is properly flattened to one line, the definition line
    // should contain both "list[" AND the corresponding "]" closing bracket.
    // With the bug, the first line only has "type Matrix = list[" -- no closing "]"
    // because the type expression spills to subsequent lines.
    //
    // A properly flattened line would look like:
    //   "...type/Matrix]: type Matrix = list[ list[float], ]"
    // The buggy line is:
    //   "...type/Matrix]: type Matrix = list["
    // which does NOT contain the "]" from the closing of list[float].
    assert!(
        def_line.contains("list[float]"),
        "multiline type alias signature should be flattened to a single line containing \
         'list[float]', but got: {}",
        def_line
    );
}

// === sample.py additional scope/kind tests ===
