use crate::model::{DefContent, DefKind};
use crate::parser::{
    LanguageParser, MatchMode, build_scope, build_scope_from_node, first_child_by_kind,
    first_line_of_node, flatten_bytes, line_range, node_text_ref, normalize_signature,
};
use tree_sitter::{Node, Parser};

pub struct JavaParser;

impl LanguageParser for JavaParser {
    fn language(&self) -> &'static str {
        "java"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &[".java"]
    }

    fn supported_kinds(&self) -> &'static [DefKind] {
        &[
            DefKind::Class,
            DefKind::Interface,
            DefKind::Enum,
            DefKind::Function,
            DefKind::Const,
        ]
    }

    impl_init_parser!(tree_sitter_java::LANGUAGE, "Java");

    impl_extract_with!(collect_definitions, scope: "");
}

/// Common skeleton for extracting a definition from an AST node.
/// Parameterized by `def_kind` and a signature extraction strategy.
fn extract_and_push_definition(
    node: Node,
    source: &str,
    mode: &MatchMode,
    def_kind: DefKind,
    sig_extractor: impl Fn(Node, &str) -> String,
    results: &mut Vec<DefContent>,
    scope: &str,
) {
    let name_node = match node.child_by_field_name("name") {
        Some(n) => n,
        None => return,
    };
    let name_ref = node_text_ref(name_node, source);

    if !mode.matches_ident(name_ref) {
        return;
    }

    let signature = sig_extractor(node, source);
    let start_row = node.start_position().row + 1;
    let [start, end] = line_range(start_row, node);

    results.push(DefContent {
        kind: def_kind,
        lines: [start, end],
        signature,
        scope: scope.to_string(),
    });
}

/// Handle a type-level definition node (class, interface, or enum).
/// Signature: up to `body` boundary, or first line as fallback.
fn handle_type_definition(
    node: Node,
    source: &str,
    mode: &MatchMode,
    def_kind: DefKind,
    results: &mut Vec<DefContent>,
    scope: &str,
) {
    extract_and_push_definition(
        node,
        source,
        mode,
        def_kind,
        |node, source| match node.child_by_field_name("body") {
            Some(body) => flatten_bytes(node.start_byte(), body.start_byte(), source)
                .unwrap_or_else(|| first_line_of_node(node, source)),
            None => first_line_of_node(node, source),
        },
        results,
        scope,
    );
}

/// Handle a callable definition node (method or constructor).
/// Signature strategy:
/// - Has `body` field: truncate to body boundary
/// - No `body` (abstract/interface method): truncate to `parameters` end boundary
/// - Fallback: first line of node
fn handle_callable(
    node: Node,
    source: &str,
    mode: &MatchMode,
    results: &mut Vec<DefContent>,
    scope: &str,
) {
    extract_and_push_definition(
        node,
        source,
        mode,
        DefKind::Function,
        |node, source| {
            let raw_sig = if let Some(body) = node.child_by_field_name("body") {
                flatten_bytes(node.start_byte(), body.start_byte(), source)
                    .unwrap_or_else(|| first_line_of_node(node, source))
            } else if let Some(params) = node.child_by_field_name("parameters") {
                let end = if let Some(throws) = first_child_by_kind(node, "throws") {
                    throws.end_byte()
                } else {
                    params.end_byte()
                };
                flatten_bytes(node.start_byte(), end, source)
                    .unwrap_or_else(|| first_line_of_node(node, source))
            } else {
                first_line_of_node(node, source)
            };
            normalize_signature(&raw_sig)
        },
        results,
        scope,
    );
}

/// Java constants must be both static and final; fields with only one modifier are mutable state.
fn is_static_final(node: Node) -> bool {
    let modifiers = match first_child_by_kind(node, "modifiers") {
        Some(m) => m,
        None => return false,
    };
    let mut has_static = false;
    let mut has_final = false;
    let mut cursor = modifiers.walk();
    for child in modifiers.children(&mut cursor) {
        match child.kind() {
            "static" => has_static = true,
            "final" => has_final = true,
            _ => {}
        }
    }
    has_static && has_final
}

/// Handle a field_declaration node, extracting static final constants only.
fn handle_field(
    node: Node,
    source: &str,
    mode: &MatchMode,
    results: &mut Vec<DefContent>,
    scope: &str,
) {
    if !is_static_final(node) {
        return;
    }
    push_declarator_definitions(node, source, mode, results, scope);
}

/// Handle a constant_declaration node (interface constants, implicitly public static final).
fn handle_interface_const(
    node: Node,
    source: &str,
    mode: &MatchMode,
    results: &mut Vec<DefContent>,
    scope: &str,
) {
    push_declarator_definitions(node, source, mode, results, scope);
}

/// Iterate over variable_declarator children of `node`, pushing a Definition
/// for each one whose name matches `mode`.
fn push_declarator_definitions(
    node: Node,
    source: &str,
    mode: &MatchMode,
    results: &mut Vec<DefContent>,
    scope: &str,
) {
    let sig = first_line_of_node(node, source);
    let start_row = node.start_position().row + 1;
    let [start, end] = line_range(start_row, node);

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() != "variable_declarator" {
            continue;
        }
        let name_node = match child.child_by_field_name("name") {
            Some(n) => n,
            None => continue,
        };
        let name_ref = node_text_ref(name_node, source);
        if !mode.matches_ident(name_ref) {
            continue;
        }

        let own_scope = build_scope(scope, ".", name_ref);
        results.push(DefContent {
            kind: DefKind::Const,
            lines: [start, end],
            signature: sig.clone(),
            scope: own_scope,
        });
    }
}

/// Recursively walk the AST, dispatching to type-specific handlers.
/// `scope` is the dot-separated context string (e.g. "Outer.Builder") built from
/// ancestor class/interface/enum nodes.
fn collect_definitions(
    node: Node,
    source: &str,
    mode: &MatchMode,
    kinds: &[DefKind],
    results: &mut Vec<DefContent>,
    scope: &str,
) {
    match node.kind() {
        "class_declaration" | "interface_declaration" => {
            let def_kind = if node.kind() == "class_declaration" {
                DefKind::Class
            } else {
                DefKind::Interface
            };
            let own_scope = build_scope_from_node(node, source, scope, ".");
            if kinds.contains(&def_kind) {
                handle_type_definition(node, source, mode, def_kind, results, &own_scope);
            }
            // Always recurse into body to discover nested types
            if let Some(body) = node.child_by_field_name("body") {
                recurse_children(body, source, mode, kinds, results, &own_scope);
            }
        }
        "enum_declaration" => {
            let own_scope = build_scope_from_node(node, source, scope, ".");
            if kinds.contains(&DefKind::Enum) {
                handle_type_definition(node, source, mode, DefKind::Enum, results, &own_scope);
            }
            // Build new scope so inner classes get the enum name.
            recurse_children(node, source, mode, kinds, results, &own_scope);
        }
        "method_declaration" | "constructor_declaration" => {
            if kinds.contains(&DefKind::Function) {
                let callable_scope = build_scope_from_node(node, source, scope, ".");
                handle_callable(node, source, mode, results, &callable_scope);
            }
            // Do not recurse into method/constructor body
        }
        "field_declaration" => {
            if kinds.contains(&DefKind::Const) {
                handle_field(node, source, mode, results, scope);
            }
            // Do not recurse -- field initializers do not contain nested definitions
            // (anonymous class bodies in initializers are intentionally excluded)
        }
        "constant_declaration" => {
            if kinds.contains(&DefKind::Const) {
                handle_interface_const(node, source, mode, results, scope);
            }
            // Do not recurse -- interface constants do not contain nested definitions
        }
        // Skip: not definitions we extract
        "package_declaration" | "import_declaration" => {}
        // Recurse into all other nodes (M2 will add specific handlers)
        _ => {
            recurse_children(node, source, mode, kinds, results, scope);
        }
    }
}

/// Recurse into all direct children of a node.
fn recurse_children(
    node: Node,
    source: &str,
    mode: &MatchMode,
    kinds: &[DefKind],
    results: &mut Vec<DefContent>,
    scope: &str,
) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_definitions(child, source, mode, kinds, results, scope);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::extract_definitions;

    // === Meta tests ===

    // === Edge case tests ===

    #[test]
    fn test_extract_with_empty_source() {
        let results = extract_definitions(&JavaParser, "anything", DefKind::all(), "");
        assert!(results.is_empty());
    }

    #[test]
    fn test_extract_with_malformed_source() {
        let results = extract_definitions(&JavaParser, "anything", DefKind::all(), "{{{{class");
        assert!(results.len() <= 10);
    }

    #[test]
    fn test_missing_body_fallback() {
        // Incomplete class (no braces) -- tree-sitter may produce ERROR or partial parse
        let results =
            extract_definitions(&JavaParser, "MyClass", &[DefKind::Class], "class MyClass");
        // Just verify no panic/crash
        assert!(results.len() <= 1);
    }

    #[test]
    fn test_annotation_in_signature() {
        let src = "@Deprecated\npublic class MyClass {}";
        let results = extract_definitions(&JavaParser, "MyClass", &[DefKind::Class], src);
        assert_eq!(results.len(), 1);
        assert!(results[0].signature.contains("@Deprecated"));
        assert!(results[0].signature.contains("public class MyClass"));
    }

    #[test]
    fn test_functional_interface_annotation() {
        let src = "@FunctionalInterface\ninterface Processor { void process(); }";
        let results = extract_definitions(&JavaParser, "Processor", &[DefKind::Interface], src);
        assert_eq!(results.len(), 1);
        assert!(results[0].signature.contains("@FunctionalInterface"));
    }

    #[test]
    fn test_class_with_complex_generics() {
        let src = "public class Container<T extends Comparable<T>> {}";
        let results = extract_definitions(&JavaParser, "Container", &[DefKind::Class], src);
        assert_eq!(results.len(), 1);
        assert!(
            results[0]
                .signature
                .contains("Container<T extends Comparable<T>>")
        );
    }

    #[test]
    fn test_enum_with_constructor_not_extracted_as_class() {
        let src = "enum Color { RED; private Color() {} }";
        let color = extract_definitions(&JavaParser, "Color", &[DefKind::Enum], src);
        assert_eq!(color.len(), 1);
        // Constructor "Color" is constructor_declaration, not class_declaration
        let class_color = extract_definitions(&JavaParser, "Color", &[DefKind::Class], src);
        assert!(class_color.is_empty());
    }

    #[test]
    fn test_package_and_import_skipped() {
        let src = "package com.example;\nimport java.util.List;\nclass App {}";
        let pkg = extract_definitions(&JavaParser, "com", DefKind::all(), src);
        assert!(pkg.is_empty());

        let imp = extract_definitions(&JavaParser, "List", DefKind::all(), src);
        assert!(imp.is_empty());

        let app = extract_definitions(&JavaParser, "App", &[DefKind::Class], src);
        assert_eq!(app.len(), 1);
    }

    #[test]
    fn test_annotation_type_not_extracted() {
        let src = "@interface MyAnnotation { String value(); }";
        let results = extract_definitions(&JavaParser, "MyAnnotation", DefKind::all(), src);
        assert!(results.is_empty());
    }

    #[test]
    fn test_multi_line_class_signature() {
        let src = "public\nabstract\nclass Shape\n{ }";
        let results = extract_definitions(&JavaParser, "Shape", &[DefKind::Class], src);
        assert_eq!(results.len(), 1);
        assert!(!results[0].signature.contains('\n'));
        assert_eq!(results[0].signature, "public abstract class Shape");
    }

    #[test]
    fn test_multi_line_method_signature() {
        let src =
            "class Foo {\n  @Override\n  public void\n  process(\n    String input\n  ) {}\n}";
        let results = extract_definitions(&JavaParser, "process", &[DefKind::Function], src);
        assert_eq!(results.len(), 1);
        assert!(!results[0].signature.contains('\n'));
        assert!(results[0].signature.contains("@Override"));
        assert!(
            results[0]
                .signature
                .contains("public void process(String input)")
        );
    }

    #[test]
    fn test_multiple_classes_same_name_different_scope() {
        let src = "class Inner {} class Outer { class Inner {} }";
        let results = extract_definitions(&JavaParser, "Inner", &[DefKind::Class], src);
        assert_eq!(results.len(), 2);
        assert!(results.iter().any(|r| r.scope == "Inner"));
        assert!(results.iter().any(|r| r.scope == "Outer.Inner"));
    }

    #[test]
    fn test_method_name_missing_no_crash() {
        let results = extract_definitions(
            &JavaParser,
            "anything",
            &[DefKind::Function],
            "class Foo { void () {} }",
        );
        assert!(results.len() <= 2);
    }

    #[test]
    fn test_no_extract_from_method_body() {
        let results = extract_definitions(
            &JavaParser,
            "InnerClass",
            &[DefKind::Class],
            "class Foo { void method() { class InnerClass {} } }",
        );
        assert!(results.is_empty());
    }

    #[test]
    fn test_kind_filter_class_not_method() {
        let results = extract_definitions(
            &JavaParser,
            "getField",
            &[DefKind::Class],
            "class Foo { int getField() { return 0; } }",
        );
        assert!(results.is_empty());
    }

    #[test]
    fn test_method_same_name_as_class() {
        let results = extract_definitions(
            &JavaParser,
            "Foo",
            &[DefKind::Function],
            "class Foo { void Foo() {} }",
        );
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].kind, DefKind::Function);
    }

    #[test]
    fn test_non_static_final_field_skipped() {
        let results = extract_definitions(
            &JavaParser,
            "field",
            &[DefKind::Const],
            "class Foo { private int field; }",
        );
        assert!(results.is_empty());
    }

    #[test]
    fn test_static_only_field_skipped() {
        let results = extract_definitions(
            &JavaParser,
            "count",
            &[DefKind::Const],
            "class Foo { static int count = 0; }",
        );
        assert!(results.is_empty());
    }

    #[test]
    fn test_final_only_field_skipped() {
        let results = extract_definitions(
            &JavaParser,
            "name",
            &[DefKind::Const],
            "class Foo { final String name; }",
        );
        assert!(results.is_empty());
    }

    #[test]
    fn test_enum_constant_not_extracted() {
        let results = extract_definitions(
            &JavaParser,
            "RED",
            &[DefKind::Const],
            "enum Color { RED, GREEN, BLUE }",
        );
        assert!(results.is_empty());
    }

    #[test]
    fn test_annotation_type_not_extracted_const() {
        let results = extract_definitions(
            &JavaParser,
            "value",
            &[DefKind::Const],
            "@interface MyAnnotation { String value() default \"\"; }",
        );
        assert!(results.is_empty());
    }
}
