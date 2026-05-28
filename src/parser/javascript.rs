use crate::model::{DefContent, DefKind};
use crate::parser::{
    LanguageParser, MatchMode, build_scope, build_scope_from_node, classify_method_definition,
    export_aware_sig_node, first_child_by_kind, first_line_of_node, flatten_bytes, handle_pair,
    handle_var_decl, line_range, node_text_ref,
};
use tree_sitter::{Node, Parser};

pub(crate) const LANGUAGE: &str = "javascript";
pub(crate) const EXTENSIONS: &[&str] = &["js", "jsx", "mjs", "cjs"];
pub(crate) const ALIASES: &[&str] = &["js", "node", "ecmascript"];

pub struct JsParser;

impl LanguageParser for JsParser {
    fn language(&self) -> &'static str {
        LANGUAGE
    }

    fn extensions(&self) -> &'static [&'static str] {
        EXTENSIONS
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
            DefKind::Field,
            DefKind::Var,
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
        "pair" => {
            handle_pair(node, source, mode, kinds, results, scope);
            return;
        }
        "field_definition" => {
            if kinds.contains(&DefKind::Field) {
                handle_js_field(node, source, mode, results, scope);
            }
            return;
        }
        "lexical_declaration" => {
            handle_lexical_decl(node, source, mode, kinds, results, scope);
            return;
        }
        "variable_declaration" => {
            handle_var_decl(node, source, mode, kinds, results, scope);
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
    let is_const = first_child_by_kind(node, "const").is_some();

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
            _ => {
                if is_const {
                    DefKind::Const
                } else {
                    DefKind::Var
                }
            }
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

/// Handle a field_definition node (class field in JS).
/// JS tree-sitter uses "property" field (not "name") for the field identifier.
fn handle_js_field<'a>(
    node: Node<'a>,
    source: &str,
    mode: &MatchMode,
    results: &mut Vec<DefContent>,
    scope: &str,
) {
    let name_node = match node.child_by_field_name("property") {
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
