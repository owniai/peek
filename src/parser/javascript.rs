use crate::model::{DefContent, DefKind};
use crate::parser::{
    LanguageParser, MatchMode, build_scope, build_scope_from_node, classify_method_definition,
    first_child_by_kind, first_line_of_node, flatten_bytes, line_range, node_text_ref,
};
use tree_sitter::{Node, Parser};

pub struct JsParser;

impl LanguageParser for JsParser {
    fn language(&self) -> &'static str {
        "js"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &[".js", ".jsx", ".mjs", ".cjs"]
    }

    fn supported_kinds(&self) -> &'static [DefKind] {
        &[
            DefKind::Function,
            DefKind::Method,
            DefKind::Constructor,
            DefKind::Getter,
            DefKind::Setter,
            DefKind::Class,
            DefKind::Const,
        ]
    }

    impl_init_parser!(tree_sitter_javascript::LANGUAGE, "JS");

    impl_extract_with!(collect_definitions, scope: "");
}

fn collect_definitions<'a>(
    node: Node<'a>,
    source: &str,
    mode: &MatchMode,
    kinds: &[DefKind],
    results: &mut Vec<DefContent>,
    scope: &str,
) {
    match node.kind() {
        "export_statement" => {
            if let Some(decl) = node.child_by_field_name("declaration") {
                collect_definitions(decl, source, mode, kinds, results, scope);
            }
            return;
        }
        "function_declaration" | "generator_function_declaration" => {
            handle_definition(
                node,
                source,
                mode,
                kinds,
                results,
                DefKind::Function,
                "statement_block",
                scope,
            );
            return;
        }
        "class_declaration" => {
            handle_definition(
                node,
                source,
                mode,
                kinds,
                results,
                DefKind::Class,
                "class_body",
                scope,
            );
            let new_scope = build_scope_from_node(node, source, scope, ".");
            recurse_into_body(node, source, mode, kinds, results, &new_scope);
            return;
        }
        "method_definition" => {
            let def_kind = classify_method_definition(node, source);
            handle_definition(
                node,
                source,
                mode,
                kinds,
                results,
                def_kind,
                "statement_block",
                scope,
            );
            return;
        }
        "lexical_declaration" => {
            handle_lexical_decl(node, source, mode, kinds, results, scope);
            return;
        }
        "ERROR" => {
            handle_error_function(node, source, mode, kinds, results, scope);
            return;
        }
        _ => {}
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_definitions(child, source, mode, kinds, results, scope);
    }
}

/// 处理 ERROR 节点中可能的不完整函数声明（如 "function foo()" 无 body）。
/// ERROR 子节点结构：function + identifier + formal_parameters（无 statement_block）。
fn handle_error_function<'a>(
    node: Node<'a>,
    source: &str,
    mode: &MatchMode,
    kinds: &[DefKind],
    results: &mut Vec<DefContent>,
    scope: &str,
) {
    if !kinds.contains(&DefKind::Function) {
        return;
    }

    if first_child_by_kind(node, "function").is_none() {
        return;
    }

    let name_node = match first_child_by_kind(node, "identifier") {
        Some(n) => n,
        None => return,
    };
    let name_ref = node_text_ref(name_node, source);

    if !mode.matches_ident(name_ref) {
        return;
    }

    let name = name_ref.to_string();
    let signature = first_line_of_node(node, source);
    let start_row = node.start_position().row + 1;
    let [start, end] = line_range(start_row, node);
    let def_scope = build_scope(scope, ".", &name);

    results.push(DefContent {
        kind: DefKind::Function,
        lines: [start, end],
        signature,
        scope: def_scope,
    });
}

/// Return the export-aware signature start node: if `node` is wrapped in an
/// `export_statement`, return that parent; otherwise return `node` itself.
fn export_aware_sig_node(node: Node) -> Node {
    match node.parent() {
        Some(p) if p.kind() == "export_statement" => p,
        _ => node,
    }
}

fn extract_signature_to_body(node: Node, source: &str, body_kind: &str) -> String {
    let sig_node = export_aware_sig_node(node);
    let body = first_child_by_kind(node, body_kind);
    let end_byte = body
        .map(|b| b.start_byte())
        .unwrap_or_else(|| node.end_byte());
    flatten_bytes(sig_node.start_byte(), end_byte, source)
        .unwrap_or_else(|| first_line_of_node(sig_node, source))
}

#[allow(clippy::too_many_arguments)]
fn handle_definition<'a>(
    node: Node<'a>,
    source: &str,
    mode: &MatchMode,
    kinds: &[DefKind],
    results: &mut Vec<DefContent>,
    def_kind: DefKind,
    body_kind: &str,
    scope: &str,
) {
    if !kinds.contains(&def_kind) {
        return;
    }

    let name_node = match node.child_by_field_name("name") {
        Some(n) => n,
        None => return,
    };
    let name_ref = node_text_ref(name_node, source);

    if !mode.matches_ident(name_ref) {
        return;
    }

    let name = name_ref.to_string();
    let signature = extract_signature_to_body(node, source, body_kind);
    let start_row = node.start_position().row + 1;
    let [start, end] = line_range(start_row, node);
    let def_scope = build_scope(scope, ".", &name);

    results.push(DefContent {
        kind: def_kind,
        lines: [start, end],
        signature,
        scope: def_scope,
    });
}

fn handle_lexical_decl<'a>(
    node: Node<'a>,
    source: &str,
    mode: &MatchMode,
    kinds: &[DefKind],
    results: &mut Vec<DefContent>,
    scope: &str,
) {
    if first_child_by_kind(node, "const").is_none() {
        return;
    }

    let sig_start_node = export_aware_sig_node(node);

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

        let value_node = child.child_by_field_name("value");
        let def_kind = match value_node.as_ref().map(|v| v.kind()) {
            Some("arrow_function") | Some("function_expression") => DefKind::Function,
            Some("class") => DefKind::Class,
            _ => DefKind::Const,
        };

        let own_scope = build_scope(scope, ".", name_ref);

        if kinds.contains(&def_kind) && mode.matches_ident(name_ref) {
            let signature = flatten_bytes(sig_start_node.start_byte(), child.end_byte(), source)
                .unwrap_or_else(|| first_line_of_node(sig_start_node, source));
            let start_row = sig_start_node.start_position().row + 1;
            let [start, end] = line_range(start_row, node);

            results.push(DefContent {
                kind: def_kind,
                lines: [start, end],
                signature,
                scope: own_scope.clone(),
            });
        }

        // Recurse into value body for class/object definitions (not function bodies)
        if let Some(value) = value_node {
            match value.kind() {
                "class" => {
                    if let Some(body) = value.child_by_field_name("body") {
                        let mut bc = body.walk();
                        for gc in body.children(&mut bc) {
                            collect_definitions(gc, source, mode, kinds, results, &own_scope);
                        }
                    }
                }
                "object" => {
                    let mut vc = value.walk();
                    for gc in value.children(&mut vc) {
                        collect_definitions(gc, source, mode, kinds, results, &own_scope);
                    }
                }
                _ => {}
            }
        }
    }
}

fn recurse_into_body(
    node: Node,
    source: &str,
    mode: &MatchMode,
    kinds: &[DefKind],
    results: &mut Vec<DefContent>,
    scope: &str,
) {
    if let Some(body) = node.child_by_field_name("body") {
        let mut cursor = body.walk();
        for child in body.children(&mut cursor) {
            collect_definitions(child, source, mode, kinds, results, scope);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::extract_definitions;

    // --- Meta tests ---

    // --- Edge case tests ---

    #[test]
    fn extract_empty_source() {
        let results = extract_definitions(&JsParser, "anything", &[DefKind::Function], "");
        assert!(results.is_empty());
    }

    #[test]
    fn malformed_source() {
        let results = extract_definitions(
            &JsParser,
            "anything",
            &[DefKind::Function],
            "function {{{{}}}}",
        );
        assert!(results.is_empty() || results.len() <= 10);
    }

    #[test]
    fn missing_body_block() {
        let results = extract_definitions(&JsParser, "foo", &[DefKind::Function], "function foo()");
        assert_eq!(results.len(), 1);
        assert!(results[0].signature.contains("function foo()"));
    }

    #[test]
    fn multi_line_signature() {
        let src = "function multi(\n  x,\n  y\n) {}";
        let results = extract_definitions(&JsParser, "multi", &[DefKind::Function], src);
        assert_eq!(results.len(), 1);
        assert!(!results[0].signature.contains('\n'));
        assert!(results[0].signature.contains("multi"));
        assert!(results[0].signature.contains("x"));
        assert!(results[0].signature.contains("y"));
    }

    #[test]
    fn line_range_correct() {
        let src = "function foo() {\n  return 1;\n}";
        let results = extract_definitions(&JsParser, "foo", &[DefKind::Function], src);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].lines, [1, 3]);
    }

    #[test]
    fn multi_line_const_signature() {
        let src = "const data = {\n  x: 1,\n  y: 2\n};";
        let results = extract_definitions(&JsParser, "data", &[DefKind::Const], src);
        assert_eq!(results.len(), 1);
        assert!(!results[0].signature.contains('\n'));
        assert!(results[0].signature.contains("const data"));
    }

    #[test]
    fn multi_line_const_arrow_signature() {
        let src = "const handler = (\n  x,\n  y\n) => {\n  return x + y;\n};";
        let results = extract_definitions(&JsParser, "handler", &[DefKind::Function], src);
        assert_eq!(results.len(), 1);
        assert!(!results[0].signature.contains('\n'));
        assert!(results[0].signature.contains("const handler"));
    }

    // --- Kind filter / disambiguation edge cases ---

    #[test]
    fn kind_filter_func_not_class() {
        let results = extract_definitions(&JsParser, "foo", &[DefKind::Class], "function foo() {}");
        assert!(results.is_empty());
    }

    // --- Exception path: let/var skipped ---

    #[test]
    fn let_skipped() {
        let results = extract_definitions(
            &JsParser,
            "letVar",
            &[DefKind::Const, DefKind::Function],
            "let letVar = 'hello';",
        );
        assert_eq!(results.len(), 0);
    }

    #[test]
    fn let_arrow_skipped() {
        let results = extract_definitions(
            &JsParser,
            "letArrow",
            &[DefKind::Function],
            "let letArrow = () => {};",
        );
        assert_eq!(results.len(), 0);
    }

    #[test]
    fn var_skipped() {
        let results = extract_definitions(
            &JsParser,
            "varVar",
            &[DefKind::Const, DefKind::Function],
            "var varVar = true;",
        );
        assert!(results.is_empty());
    }

    #[test]
    fn const_destructure_substring_match() {
        // Substring matching: "a" appears in "{ a, b }" (the name text of destructured const)
        let results =
            extract_definitions(&JsParser, "a", &[DefKind::Const], "const { a, b } = obj;");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].kind, DefKind::Const);
    }

    #[test]
    fn anonymous_arrow_not_extracted() {
        let results = extract_definitions(
            &JsParser,
            "callback",
            &[DefKind::Function],
            "setTimeout(() => { console.log('hi'); }, 1000);",
        );
        assert!(results.is_empty());
    }

    // --- Object literal method scope edge cases ---

    #[test]
    fn object_literal_method_scope() {
        let src = "const config = { init() { return 1; } };";
        let results = extract_definitions(&JsParser, "init", &[DefKind::Method], src);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].kind, DefKind::Method);
        assert_eq!(results[0].scope, "config.init");
    }

    #[test]
    fn object_literal_method_nested_in_function_not_extracted() {
        let src = "function setup() { const config = { init() {} }; }";
        let results = extract_definitions(&JsParser, "init", &[DefKind::Function], src);
        assert!(
            results.is_empty(),
            "Function-body definitions should not be extracted, got: {results:?}"
        );
    }

    #[test]
    fn object_literal_multiple_methods() {
        let src = "const obj = { init() {}, destroy() {} };";
        let init_results = extract_definitions(&JsParser, "init", &[DefKind::Method], src);
        assert_eq!(init_results.len(), 1);
        assert_eq!(init_results[0].scope, "obj.init");

        let destroy_results = extract_definitions(&JsParser, "destroy", &[DefKind::Method], src);
        assert_eq!(destroy_results.len(), 1);
        assert_eq!(destroy_results[0].scope, "obj.destroy");
    }

    // --- Scope: function_expression / arrow_function contribute to scope ---

    // --- Function-body definitions should NOT be extracted ---

    #[test]
    fn scope_nested_in_function_expression_not_extracted() {
        let src = "const fn = function() { function inner() {} };";
        let results = extract_definitions(&JsParser, "inner", &[DefKind::Function], src);
        assert!(
            results.is_empty(),
            "Function-body definitions should not be extracted, got: {results:?}"
        );
    }

    #[test]
    fn scope_nested_in_arrow_function_not_extracted() {
        let src = "const fn = () => { function nested() {} };";
        let results = extract_definitions(&JsParser, "nested", &[DefKind::Function], src);
        assert!(
            results.is_empty(),
            "Function-body definitions should not be extracted, got: {results:?}"
        );
    }

    // --- Kind: class_expression identified as Class ---

    #[test]
    fn class_expression_kind() {
        let src = "const MyClass = class { method() {} };";
        let results = extract_definitions(&JsParser, "MyClass", &[DefKind::Class], src);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].kind, DefKind::Class);
        assert_eq!(results[0].scope, "MyClass");
    }

    #[test]
    fn class_expression_method_scope() {
        let src = "const MyClass = class { method() {} };";
        let results = extract_definitions(&JsParser, "method", &[DefKind::Method], src);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].scope, "MyClass.method");
    }

    // --- Sub-kind classification for method_definition ---

    #[test]
    fn class_method_is_method_kind() {
        let src = "class Foo { bar() {} }";
        let results = extract_definitions(&JsParser, "bar", &[DefKind::Method], src);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].kind, DefKind::Method);
        assert_eq!(results[0].scope, "Foo.bar");
    }

    #[test]
    fn class_constructor_is_constructor_kind() {
        let src = "class Foo { constructor() {} }";
        let results = extract_definitions(&JsParser, "constructor", &[DefKind::Constructor], src);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].kind, DefKind::Constructor);
        assert_eq!(results[0].scope, "Foo.constructor");
    }

    #[test]
    fn class_getter_is_getter_kind() {
        let src = "class Foo { get name() { return 1; } }";
        let results = extract_definitions(&JsParser, "name", &[DefKind::Getter], src);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].kind, DefKind::Getter);
        assert_eq!(results[0].scope, "Foo.name");
    }

    #[test]
    fn class_setter_is_setter_kind() {
        let src = "class Foo { set name(v) { this._name = v; } }";
        let results = extract_definitions(&JsParser, "name", &[DefKind::Setter], src);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].kind, DefKind::Setter);
        assert_eq!(results[0].scope, "Foo.name");
    }

    #[test]
    fn object_literal_method_is_method_kind() {
        let src = "const obj = { init() {} };";
        let results = extract_definitions(&JsParser, "init", &[DefKind::Method], src);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].kind, DefKind::Method);
        assert_eq!(results[0].scope, "obj.init");
    }

    #[test]
    fn object_literal_getter_is_getter_kind() {
        let src = "const obj = { get x() { return 1; } };";
        let results = extract_definitions(&JsParser, "x", &[DefKind::Getter], src);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].kind, DefKind::Getter);
        assert_eq!(results[0].scope, "obj.x");
    }

    #[test]
    fn object_literal_setter_is_setter_kind() {
        let src = "const obj = { set x(v) {} };";
        let results = extract_definitions(&JsParser, "x", &[DefKind::Setter], src);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].kind, DefKind::Setter);
        assert_eq!(results[0].scope, "obj.x");
    }

    #[test]
    fn method_kind_excludes_top_level_function() {
        let src = "function foo() {}";
        let results = extract_definitions(&JsParser, "foo", &[DefKind::Method], src);
        assert!(results.is_empty());
    }

    #[test]
    fn function_kind_excludes_class_method() {
        let src = "class Foo { bar() {} }";
        let results = extract_definitions(&JsParser, "bar", &[DefKind::Function], src);
        assert!(results.is_empty());
    }

    #[test]
    fn callable_includes_all_sub_kinds() {
        let src = "function foo() {} class Bar { constructor() {} baz() {} get x() { return 1; } set x(v) {} }";
        let all_callables = &[
            DefKind::Function,
            DefKind::Method,
            DefKind::Constructor,
            DefKind::Getter,
            DefKind::Setter,
        ];
        let foo = extract_definitions(&JsParser, "foo", all_callables, src);
        assert_eq!(foo.len(), 1);
        assert_eq!(foo[0].kind, DefKind::Function);

        let ctor = extract_definitions(&JsParser, "constructor", all_callables, src);
        assert_eq!(ctor.len(), 1);
        assert_eq!(ctor[0].kind, DefKind::Constructor);

        let baz = extract_definitions(&JsParser, "baz", all_callables, src);
        assert_eq!(baz.len(), 1);
        assert_eq!(baz[0].kind, DefKind::Method);

        let getter = extract_definitions(&JsParser, "x", all_callables, src);
        assert_eq!(getter.len(), 2); // getter + setter both match "x"
        assert!(getter.iter().any(|r| r.kind == DefKind::Getter));
        assert!(getter.iter().any(|r| r.kind == DefKind::Setter));
    }
}
