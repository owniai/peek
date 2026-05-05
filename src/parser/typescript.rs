use crate::model::{DefContent, DefKind};
use crate::parser::{
    LanguageParser, MatchMode, build_scope, build_scope_from_node, classify_method_definition,
    first_child_by_kind, first_line_of_node, flatten_bytes, line_range, node_text_ref,
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
            DefKind::Method,
            DefKind::Constructor,
            DefKind::Getter,
            DefKind::Setter,
            DefKind::Class,
            DefKind::Const,
            DefKind::Interface,
            DefKind::Alias,
            DefKind::Enum,
            DefKind::Field,
            DefKind::Property,
            DefKind::Namespace,
            DefKind::Variant,
            DefKind::Subscript,
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

/// Handle a public_field_definition node (class field in TS).
fn handle_field<'a>(
    node: Node<'a>,
    source: &str,
    mode: &MatchMode,
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

    let own_scope = build_scope(scope, ".", name_ref);
    let signature = first_line_of_node(node, source);
    let start_row = node.start_position().row + 1;
    let [start, end] = line_range(start_row, node);

    results.push(DefContent {
        kind: DefKind::Field,
        lines: [start, end],
        signature,
        scope: own_scope,
    });
}

/// Handle a property_signature node (interface property in TS).
fn handle_property_signature<'a>(
    node: Node<'a>,
    source: &str,
    mode: &MatchMode,
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

    let own_scope = build_scope(scope, ".", name_ref);
    let signature = first_line_of_node(node, source);
    let start_row = node.start_position().row + 1;
    let [start, end] = line_range(start_row, node);

    results.push(DefContent {
        kind: DefKind::Property,
        lines: [start, end],
        signature,
        scope: own_scope,
    });
}

fn handle_type_alias<'a>(
    node: Node<'a>,
    source: &str,
    mode: &MatchMode,
    kinds: &[DefKind],
    results: &mut Vec<DefContent>,
    scope: &str,
) {
    if !kinds.contains(&DefKind::Alias) {
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
        kind: DefKind::Alias,
        lines: [start, end],
        signature,
        scope: def_scope,
    });
}

/// Handle a property_identifier node inside an enum_body (TS enum member / Variant).
fn handle_enum_variant<'a>(
    node: Node<'a>,
    source: &str,
    mode: &MatchMode,
    results: &mut Vec<DefContent>,
    scope: &str,
) {
    let name_ref = node_text_ref(node, source);
    if !mode.matches_ident(name_ref) {
        return;
    }
    let name = name_ref.to_string();
    let own_scope = build_scope(scope, ".", &name);
    let signature = first_line_of_node(node, source);
    let start_row = node.start_position().row + 1;
    let [start, end] = line_range(start_row, node);

    results.push(DefContent {
        kind: DefKind::Variant,
        lines: [start, end],
        signature,
        scope: own_scope,
    });
}

/// Handle an index_signature node (TS `[key: string]: T` — Subscript kind).
fn handle_index_signature<'a>(
    node: Node<'a>,
    source: &str,
    mode: &MatchMode,
    results: &mut Vec<DefContent>,
    scope: &str,
) {
    // Build display name from the bracket pattern, e.g. "[key: string]"
    let name = extract_index_signature_name(node, source);
    if !mode.matches_ident(&name) {
        return;
    }
    let own_scope = build_scope(scope, ".", &name);
    let signature = first_line_of_node(node, source);
    let start_row = node.start_position().row + 1;
    let [start, end] = line_range(start_row, node);

    results.push(DefContent {
        kind: DefKind::Subscript,
        lines: [start, end],
        signature,
        scope: own_scope,
    });
}

/// Extract the display name from an index_signature node.
/// Returns the bracket pattern like "[key: string]" by finding the
/// text between the opening "[" and closing "]".
fn extract_index_signature_name(node: Node, source: &str) -> String {
    let mut cursor = node.walk();
    let mut start = None;
    let mut end = None;
    for child in node.children(&mut cursor) {
        if child.kind() == "[" {
            start = Some(child.start_byte());
        }
        if child.kind() == "]" {
            end = Some(child.end_byte());
            break;
        }
    }
    match (start, end) {
        (Some(s), Some(e)) => source[s..e].to_string(),
        _ => "[index]".to_string(),
    }
}

/// Handle a construct_signature node (`new(arg: T): RetType` — Constructor kind).
fn handle_construct_signature<'a>(
    node: Node<'a>,
    source: &str,
    mode: &MatchMode,
    results: &mut Vec<DefContent>,
    scope: &str,
) {
    let name = "new";
    if !mode.matches_ident(name) {
        return;
    }
    let own_scope = build_scope(scope, ".", name);
    let signature = first_line_of_node(node, source);
    let start_row = node.start_position().row + 1;
    let [start, end] = line_range(start_row, node);

    results.push(DefContent {
        kind: DefKind::Constructor,
        lines: [start, end],
        signature,
        scope: own_scope,
    });
}

/// Handle a call_signature node (`(arg: T): RetType` — Method kind).
fn handle_call_signature<'a>(
    node: Node<'a>,
    source: &str,
    mode: &MatchMode,
    results: &mut Vec<DefContent>,
    scope: &str,
) {
    let name = "call";
    if !mode.matches_ident(name) {
        return;
    }
    let own_scope = build_scope(scope, ".", name);
    let signature = first_line_of_node(node, source);
    let start_row = node.start_position().row + 1;
    let [start, end] = line_range(start_row, node);

    results.push(DefContent {
        kind: DefKind::Method,
        lines: [start, end],
        signature,
        scope: own_scope,
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
            let enum_scope = build_scope_from_node(node, source, scope, ".");
            if let Some(body) = first_child_by_kind(node, "enum_body") {
                let mut cursor = body.walk();
                for child in body.children(&mut cursor) {
                    collect_definitions(child, source, mode, kinds, results, &enum_scope);
                }
            }
            return;
        }
        "lexical_declaration" => {
            handle_lexical_decl(node, source, mode, kinds, results, scope);
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
        "method_signature" | "abstract_method_signature" => {
            handle_definition(
                node,
                source,
                mode,
                kinds,
                results,
                DefKind::Method,
                "statement_block",
                scope,
            );
            return;
        }
        "internal_module" | "module" => {
            let new_scope = build_scope_from_node(node, source, scope, ".");
            if kinds.contains(&DefKind::Namespace) && mode.matches_ident(&new_scope) {
                let sig = extract_signature_to_body(node, source, "statement_block");
                let start_row = node.start_position().row + 1;
                let [start, end] = line_range(start_row, node);
                results.push(DefContent {
                    kind: DefKind::Namespace,
                    lines: [start, end],
                    signature: sig,
                    scope: new_scope.clone(),
                });
            }
            recurse_into_body(node, source, mode, kinds, results, &new_scope);
            return;
        }
        "ERROR" => {
            handle_error_function(node, source, mode, kinds, results, scope);
            return;
        }
        "public_field_definition" => {
            if kinds.contains(&DefKind::Field) {
                handle_field(node, source, mode, results, scope);
            }
            return;
        }
        "property_signature" => {
            if kinds.contains(&DefKind::Property) {
                handle_property_signature(node, source, mode, results, scope);
            }
            return;
        }
        "property_identifier" => {
            // Inside enum_body, property_identifier nodes are enum members (Variant kind).
            // Verify parent is enum_body to avoid extracting other property_identifier nodes.
            if kinds.contains(&DefKind::Variant) {
                if let Some(parent) = node.parent() {
                    if parent.kind() == "enum_body" {
                        handle_enum_variant(node, source, mode, results, scope);
                    }
                }
            }
            return;
        }
        "enum_assignment" => {
            // enum_assignment wraps a property_identifier with an initializer value.
            // Extract the inner property_identifier as a Variant.
            if kinds.contains(&DefKind::Variant) {
                if let Some(name_node) = node.child_by_field_name("name") {
                    handle_enum_variant(name_node, source, mode, results, scope);
                } else {
                    // Fallback: find first property_identifier child
                    if let Some(pi) = first_child_by_kind(node, "property_identifier") {
                        handle_enum_variant(pi, source, mode, results, scope);
                    }
                }
            }
            return;
        }
        "index_signature" => {
            if kinds.contains(&DefKind::Subscript) {
                handle_index_signature(node, source, mode, results, scope);
            }
            return;
        }
        "construct_signature" => {
            if kinds.contains(&DefKind::Constructor) {
                handle_construct_signature(node, source, mode, results, scope);
            }
            return;
        }
        "call_signature" => {
            if kinds.contains(&DefKind::Method) {
                handle_call_signature(node, source, mode, results, scope);
            }
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

    // --- Kind filter / disambiguation edge cases ---

    #[test]
    fn ts_kind_filter_func_not_class() {
        let results = extract_definitions(&TsParser, "foo", &[DefKind::Class], "function foo() {}");
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
    fn ts_destructured_const_substring_match() {
        // Substring matching: "a" appears in "{ a, b }" (the name text of destructured const)
        let results =
            extract_definitions(&TsParser, "a", &[DefKind::Const], "const { a, b } = obj;");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].kind, DefKind::Const);
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
            &[DefKind::Alias],
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
            &[DefKind::Method],
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
        let results = extract_definitions(&TsParser, "method", &[DefKind::Method], src);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].scope, "MyClass.method");
    }

    // --- Scope: object literal method in const scope ---

    #[test]
    fn ts_object_literal_method_scope() {
        let src = "const config = { init() { return 1; } };";
        let results = extract_definitions(&TsParser, "init", &[DefKind::Method], src);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].scope, "config.init");
    }

    #[test]
    fn ts_object_literal_method_nested_in_function_not_extracted() {
        let src = "function setup() { const config = { init() {} }; }";
        let results = extract_definitions(&TsParser, "init", &[DefKind::Method], src);
        assert!(
            results.is_empty(),
            "Function-body definitions should not be extracted, got: {results:?}"
        );
    }

    // --- Sub-kind classification for method_definition ---

    #[test]
    fn ts_class_method_is_method_kind() {
        let src = "class Foo { bar(): void {} }";
        let results = extract_definitions(&TsParser, "bar", &[DefKind::Method], src);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].kind, DefKind::Method);
        assert_eq!(results[0].scope, "Foo.bar");
    }

    #[test]
    fn ts_class_constructor_is_constructor_kind() {
        let src = "class Foo { constructor() {} }";
        let results = extract_definitions(&TsParser, "constructor", &[DefKind::Constructor], src);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].kind, DefKind::Constructor);
    }

    #[test]
    fn ts_class_getter_is_getter_kind() {
        let src = "class Foo { get name(): string { return ''; } }";
        let results = extract_definitions(&TsParser, "name", &[DefKind::Getter], src);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].kind, DefKind::Getter);
    }

    #[test]
    fn ts_class_setter_is_setter_kind() {
        let src = "class Foo { set name(v: string) {} }";
        let results = extract_definitions(&TsParser, "name", &[DefKind::Setter], src);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].kind, DefKind::Setter);
    }

    // --- Sub-kind classification for method_signature / abstract_method_signature ---

    #[test]
    fn ts_method_signature_is_method_kind() {
        let src = "interface IFoo { bar(): void; }";
        let results = extract_definitions(&TsParser, "bar", &[DefKind::Method], src);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].kind, DefKind::Method);
        assert_eq!(results[0].scope, "IFoo.bar");
    }

    #[test]
    fn ts_abstract_method_signature_is_method_kind() {
        let src = "abstract class Base { doWork(): void; }";
        let results = extract_definitions(&TsParser, "doWork", &[DefKind::Method], src);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].kind, DefKind::Method);
        assert_eq!(results[0].scope, "Base.doWork");
    }

    #[test]
    fn ts_function_kind_excludes_method() {
        let src = "class Foo { bar(): void {} }";
        let results = extract_definitions(&TsParser, "bar", &[DefKind::Function], src);
        assert!(results.is_empty());
    }

    // --- Variant (enum member) tests ---

    #[test]
    fn ts_enum_variant_simple() {
        let src = "enum Color { Red, Green, Blue }";
        let results = extract_definitions(&TsParser, "Red", &[DefKind::Variant], src);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].kind, DefKind::Variant);
        assert_eq!(results[0].scope, "Color.Red");
    }

    #[test]
    fn ts_enum_variant_with_initializer() {
        let src = "enum Status { Active = 1, Inactive = 0 }";
        let results = extract_definitions(&TsParser, "Active", &[DefKind::Variant], src);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].kind, DefKind::Variant);
        assert_eq!(results[0].scope, "Status.Active");
    }

    #[test]
    fn ts_enum_variants_all_extracted() {
        let src = "enum Direction { Up, Down, Left, Right }";
        let results = extract_definitions(&TsParser, ".*", &[DefKind::Enum, DefKind::Variant], src);
        // 1 enum + 4 variants
        assert_eq!(results.len(), 5);
        assert!(
            results
                .iter()
                .any(|d| d.kind == DefKind::Enum && d.scope == "Direction")
        );
        assert!(
            results
                .iter()
                .any(|d| d.kind == DefKind::Variant && d.scope == "Direction.Up")
        );
        assert!(
            results
                .iter()
                .any(|d| d.kind == DefKind::Variant && d.scope == "Direction.Down")
        );
        assert!(
            results
                .iter()
                .any(|d| d.kind == DefKind::Variant && d.scope == "Direction.Left")
        );
        assert!(
            results
                .iter()
                .any(|d| d.kind == DefKind::Variant && d.scope == "Direction.Right")
        );
    }

    #[test]
    fn ts_enum_variant_kind_filter() {
        let src = "enum Color { Red, Green }";
        let results = extract_definitions(&TsParser, "Red", &[DefKind::Enum], src);
        assert!(
            results.is_empty(),
            "Variant should not be extracted when only Enum kind is requested, got: {results:?}"
        );
    }

    #[test]
    fn ts_enum_variant_filter_by_name() {
        let src = "enum Color { Red, Green, Blue }";
        let results = extract_definitions(&TsParser, "Green", &[DefKind::Variant], src);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].scope, "Color.Green");
    }

    // --- Subscript (index signature) tests ---

    #[test]
    fn ts_index_signature_in_interface() {
        let src = "interface StringMap { [key: string]: string; }";
        let results = extract_definitions(&TsParser, ".*", &[DefKind::Subscript], src);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].kind, DefKind::Subscript);
        assert_eq!(results[0].scope, "StringMap.[key: string]");
        assert!(results[0].signature.contains("[key: string]"));
    }

    #[test]
    fn ts_index_signature_kind_filter() {
        let src = "interface StringMap { [key: string]: string; }";
        let results = extract_definitions(&TsParser, ".*", &[DefKind::Method], src);
        assert!(
            results.is_empty(),
            "Index signature should not be extracted when only Method kind is requested, got: {results:?}"
        );
    }

    // --- construct_signature -> Constructor tests ---

    #[test]
    fn ts_construct_signature_in_interface() {
        let src = "interface Factory { new(arg: string): MyClass; }";
        let results = extract_definitions(&TsParser, ".*", &[DefKind::Constructor], src);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].kind, DefKind::Constructor);
        assert_eq!(results[0].scope, "Factory.new");
        assert!(results[0].signature.contains("new"));
    }

    #[test]
    fn ts_construct_signature_kind_filter() {
        let src = "interface Factory { new(arg: string): MyClass; }";
        let results = extract_definitions(&TsParser, ".*", &[DefKind::Method], src);
        assert!(
            results.is_empty(),
            "construct_signature should not be extracted when only Method kind is requested, got: {results:?}"
        );
    }

    // --- call_signature -> Method tests ---

    #[test]
    fn ts_call_signature_in_interface() {
        let src = "interface Callable { (arg: string): void; }";
        let results = extract_definitions(&TsParser, ".*", &[DefKind::Method], src);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].kind, DefKind::Method);
        assert_eq!(results[0].scope, "Callable.call");
    }

    #[test]
    fn ts_call_signature_kind_filter() {
        let src = "interface Callable { (arg: string): void; }";
        let results = extract_definitions(&TsParser, ".*", &[DefKind::Function], src);
        assert!(
            results.is_empty(),
            "call_signature should not be extracted when only Function kind is requested, got: {results:?}"
        );
    }

    // --- Combined interface members test ---

    #[test]
    fn ts_interface_with_mixed_members() {
        let src = "interface Container { name: string; doWork(): void; [key: string]: any; new(x: number): Container; (input: string): void; }";
        let results = extract_definitions(
            &TsParser,
            ".*",
            &[
                DefKind::Interface,
                DefKind::Property,
                DefKind::Method,
                DefKind::Subscript,
                DefKind::Constructor,
            ],
            src,
        );
        // 1 interface + 1 property + 1 method (doWork) + 1 subscript + 1 constructor (new) + 1 method (call)
        assert!(
            results
                .iter()
                .any(|d| d.kind == DefKind::Interface && d.scope == "Container")
        );
        assert!(
            results
                .iter()
                .any(|d| d.kind == DefKind::Property && d.scope == "Container.name")
        );
        assert!(
            results
                .iter()
                .any(|d| d.kind == DefKind::Method && d.scope == "Container.doWork")
        );
        assert!(
            results
                .iter()
                .any(|d| d.kind == DefKind::Subscript && d.scope == "Container.[key: string]")
        );
        assert!(
            results
                .iter()
                .any(|d| d.kind == DefKind::Constructor && d.scope == "Container.new")
        );
        assert!(
            results
                .iter()
                .any(|d| d.kind == DefKind::Method && d.scope == "Container.call")
        );
    }
}
