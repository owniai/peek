use crate::model::{DefContent, DefKind};
use crate::parser::{
    LanguageParser, MatchMode, build_scope, build_scope_from_node, first_child_by_kind,
    first_line_of_node, flatten_bytes, line_range, node_text_ref,
};
use tree_sitter::{Node, Parser};

pub struct TsParser;

impl LanguageParser for TsParser {
    fn language(&self) -> &'static str {
        "ts"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &[".ts", ".tsx", ".mts", ".cts"]
    }

    fn supported_kinds(&self) -> &'static [DefKind] {
        &[
            DefKind::Function,
            DefKind::Class,
            DefKind::Const,
            DefKind::Interface,
            DefKind::Type,
            DefKind::Enum,
        ]
    }

    impl_init_parser!(tree_sitter_typescript::LANGUAGE_TSX, "TypeScript");

    impl_extract_with!(collect_definitions, scope: "");
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

/// Handle ERROR nodes that may contain incomplete function declarations.
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

fn handle_type_alias<'a>(
    node: Node<'a>,
    source: &str,
    mode: &MatchMode,
    kinds: &[DefKind],
    results: &mut Vec<DefContent>,
    scope: &str,
) {
    if !kinds.contains(&DefKind::Type) {
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
    let sig_node = export_aware_sig_node(node);
    let signature = flatten_bytes(sig_node.start_byte(), node.end_byte(), source)
        .unwrap_or_else(|| first_line_of_node(sig_node, source));
    let start_row = sig_node.start_position().row + 1;
    let [start, end] = line_range(start_row, node);
    let def_scope = build_scope(scope, ".", &name);

    results.push(DefContent {
        kind: DefKind::Type,
        lines: [start, end],
        signature,
        scope: def_scope,
    });
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
        "function_signature" => {
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
        "abstract_class_declaration" => {
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
        "interface_declaration" => {
            handle_definition(
                node,
                source,
                mode,
                kinds,
                results,
                DefKind::Interface,
                "interface_body",
                scope,
            );
            let new_scope = build_scope_from_node(node, source, scope, ".");
            recurse_into_body(node, source, mode, kinds, results, &new_scope);
            return;
        }
        "type_alias_declaration" => {
            handle_type_alias(node, source, mode, kinds, results, scope);
            return;
        }
        "enum_declaration" => {
            handle_definition(
                node,
                source,
                mode,
                kinds,
                results,
                DefKind::Enum,
                "enum_body",
                scope,
            );
            return;
        }
        "lexical_declaration" => {
            handle_lexical_decl(node, source, mode, kinds, results, scope);
            return;
        }
        "method_definition" => {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::extract_definitions;

    #[test]
    fn language_returns_ts() {
        let p = TsParser;
        assert_eq!(p.language(), "ts");
    }

    #[test]
    fn extensions_cover_ts_tsx() {
        let p = TsParser;
        assert!(p.extensions().contains(&".ts"));
        assert!(p.extensions().contains(&".tsx"));
        assert!(p.extensions().contains(&".mts"));
        assert!(p.extensions().contains(&".cts"));
    }

    #[test]
    fn supported_kinds_six() {
        let p = TsParser;
        let kinds = p.supported_kinds();
        assert!(kinds.contains(&DefKind::Function));
        assert!(kinds.contains(&DefKind::Class));
        assert!(kinds.contains(&DefKind::Const));
        assert!(kinds.contains(&DefKind::Interface));
        assert!(kinds.contains(&DefKind::Type));
        assert!(kinds.contains(&DefKind::Enum));
    }

    // --- Kind filter / disambiguation edge cases ---

    #[test]
    fn ts_kind_filter_func_not_class() {
        let results = extract_definitions(&TsParser, "foo", &[DefKind::Class], "function foo() {}");
        assert!(results.is_empty());
    }

    #[test]
    fn ts_kind_filter_class_not_func() {
        let results = extract_definitions(&TsParser, "Foo", &[DefKind::Function], "class Foo {}");
        assert!(results.is_empty());
    }

    #[test]
    fn ts_nonexistent() {
        let results = extract_definitions(&TsParser, "bar", DefKind::all(), "function foo() {}");
        assert!(results.is_empty());
    }

    #[test]
    fn ts_kind_filter_const_not_func() {
        let results = extract_definitions(&TsParser, "v", &[DefKind::Function], "const v = 42;");
        assert!(results.is_empty());
    }

    #[test]
    fn ts_kind_filter_arrow_not_const() {
        let results =
            extract_definitions(&TsParser, "fn", &[DefKind::Const], "const fn = () => {};");
        assert!(results.is_empty());
    }

    // --- Edge case tests ---

    #[test]
    fn ts_empty_source() {
        let results = extract_definitions(&TsParser, "anything", DefKind::all(), "");
        assert!(results.is_empty());
    }

    #[test]
    fn ts_malformed_source() {
        let results =
            extract_definitions(&TsParser, "anything", DefKind::all(), "function {{{{}}}}");
        assert!(results.is_empty() || results.len() <= 10);
    }

    #[test]
    fn ts_incomplete_func() {
        let results = extract_definitions(&TsParser, "foo", &[DefKind::Function], "function foo()");
        assert_eq!(results.len(), 1);
        assert!(results[0].signature.contains("function foo()"));
    }

    #[test]
    fn ts_multi_line_func_signature() {
        let src = "function multi(\n  x: number,\n  y: string\n): void {}";
        let results = extract_definitions(&TsParser, "multi", &[DefKind::Function], src);
        assert_eq!(results.len(), 1);
        assert!(!results[0].signature.contains('\n'));
        assert!(results[0].signature.contains("multi"));
    }

    #[test]
    fn ts_line_range_correct() {
        let src = "function foo(): void {\n  return;\n}";
        let results = extract_definitions(&TsParser, "foo", &[DefKind::Function], src);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].lines, [1, 3]);
    }

    #[test]
    fn ts_destructured_const_skipped() {
        let results =
            extract_definitions(&TsParser, "a", &[DefKind::Const], "const { a, b } = obj;");
        assert!(results.is_empty());
    }

    #[test]
    fn ts_anonymous_arrow_not_extracted() {
        let results = extract_definitions(
            &TsParser,
            "cb",
            &[DefKind::Function],
            "setTimeout(() => { console.log('hi'); }, 1000);",
        );
        assert!(results.is_empty());
    }

    #[test]
    fn ts_let_skipped() {
        let results = extract_definitions(
            &TsParser,
            "v",
            &[DefKind::Const, DefKind::Function],
            "let v: number = 1;",
        );
        assert!(results.is_empty());
    }

    #[test]
    fn ts_var_skipped() {
        let results = extract_definitions(
            &TsParser,
            "v",
            &[DefKind::Const, DefKind::Function],
            "var v = 1;",
        );
        assert!(results.is_empty());
    }

    // --- Scope edge cases ---

    // --- Function-body definitions should NOT be extracted ---

    #[test]
    fn ts_interface_in_func_not_extracted() {
        let results = extract_definitions(
            &TsParser,
            "InnerIfc",
            &[DefKind::Interface],
            "function outer() { interface InnerIfc {} }",
        );
        assert!(
            results.is_empty(),
            "Function-body definitions should not be extracted, got: {results:?}"
        );
    }

    #[test]
    fn ts_enum_in_class_method_not_extracted() {
        let results = extract_definitions(
            &TsParser,
            "Kind",
            &[DefKind::Enum],
            "class Container { method() { enum Kind { A, B } } }",
        );
        assert!(
            results.is_empty(),
            "Function-body definitions should not be extracted, got: {results:?}"
        );
    }

    #[test]
    fn ts_type_in_func_not_extracted() {
        let results = extract_definitions(
            &TsParser,
            "LocalType",
            &[DefKind::Type],
            "function process() { type LocalType = string; }",
        );
        assert!(
            results.is_empty(),
            "Function-body definitions should not be extracted, got: {results:?}"
        );
    }

    #[test]
    fn ts_abstract_class_method_scope() {
        let results = extract_definitions(
            &TsParser,
            "impl",
            &[DefKind::Function],
            "abstract class Base { impl(): void {} }",
        );
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].scope, "Base.impl");
    }

    // --- TSX grammar switching tests ---

    #[test]
    fn tsx_function_component() {
        let src = "function App() { return <div>Hello</div>; }";
        let results = extract_definitions(&TsParser, "App", &[DefKind::Function], src);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].kind, DefKind::Function);
        assert_eq!(results[0].scope, "App");
    }

    #[test]
    fn tsx_arrow_component() {
        let src = "const Button = (props: { label: string }) => <button>{props.label}</button>;";
        let results = extract_definitions(&TsParser, "Button", &[DefKind::Function], src);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].kind, DefKind::Function);
    }

    #[test]
    fn tsx_class_component() {
        let src = "class App extends React.Component { render() { return <div />; } }";
        let results = extract_definitions(&TsParser, "App", &[DefKind::Class], src);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].kind, DefKind::Class);
    }

    #[test]
    fn tsx_interface() {
        let src = "interface Props { children: React.ReactNode; }";
        let results = extract_definitions(&TsParser, "Props", &[DefKind::Interface], src);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].kind, DefKind::Interface);
    }

    #[test]
    fn tsx_enum() {
        let src = "enum Theme { Light, Dark }";
        let results = extract_definitions(&TsParser, "Theme", &[DefKind::Enum], src);
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn tsx_export_default_func() {
        let src = "export default function Page() { return <main />; }";
        let results = extract_definitions(&TsParser, "Page", &[DefKind::Function], src);
        assert_eq!(results.len(), 1);
        assert!(results[0].signature.contains("export default"));
    }

    #[test]
    fn tsx_nested_definitions_not_extracted() {
        let src =
            "function App() { interface Config { debug: boolean; } const handler = () => {}; }";
        let ifc = extract_definitions(&TsParser, "Config", &[DefKind::Interface], src);
        assert!(
            ifc.is_empty(),
            "Function-body definitions should not be extracted, got: {ifc:?}"
        );

        let handler = extract_definitions(&TsParser, "handler", &[DefKind::Function], src);
        assert!(
            handler.is_empty(),
            "Function-body definitions should not be extracted, got: {handler:?}"
        );
    }

    #[test]
    fn tsx_generic_component() {
        let src = "function List<T>(props: { items: T[] }) { return <ul />; }";
        let results = extract_definitions(&TsParser, "List", &[DefKind::Function], src);
        assert_eq!(results.len(), 1);
        assert!(results[0].signature.contains("List<T>"));
    }

    // --- Scope: const arrow/function_expression body recursion ---

    #[test]
    fn ts_scope_nested_in_const_arrow_not_extracted() {
        let src = "function outer() { const inner = () => { function deep() {} }; }";
        let results = extract_definitions(&TsParser, "deep", &[DefKind::Function], src);
        assert!(
            results.is_empty(),
            "Function-body definitions should not be extracted, got: {results:?}"
        );
    }

    #[test]
    fn ts_scope_nested_in_const_function_expression_not_extracted() {
        let src = "function outer() { const inner = function() { function deep() {} }; }";
        let results = extract_definitions(&TsParser, "deep", &[DefKind::Function], src);
        assert!(
            results.is_empty(),
            "Function-body definitions should not be extracted, got: {results:?}"
        );
    }

    // --- Kind: class_expression identified as Class ---

    #[test]
    fn ts_class_expression_kind() {
        let src = "const MyClass = class { method() {} };";
        let results = extract_definitions(&TsParser, "MyClass", &[DefKind::Class], src);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].kind, DefKind::Class);
        assert_eq!(results[0].scope, "MyClass");
    }

    #[test]
    fn ts_class_expression_method_scope() {
        let src = "const MyClass = class { method() {} };";
        let results = extract_definitions(&TsParser, "method", &[DefKind::Function], src);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].scope, "MyClass.method");
    }

    // --- Scope: object literal method in const scope ---

    #[test]
    fn ts_object_literal_method_scope() {
        let src = "const config = { init() { return 1; } };";
        let results = extract_definitions(&TsParser, "init", &[DefKind::Function], src);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].scope, "config.init");
    }

    #[test]
    fn ts_object_literal_method_nested_in_function_not_extracted() {
        let src = "function setup() { const config = { init() {} }; }";
        let results = extract_definitions(&TsParser, "init", &[DefKind::Function], src);
        assert!(
            results.is_empty(),
            "Function-body definitions should not be extracted, got: {results:?}"
        );
    }
}
