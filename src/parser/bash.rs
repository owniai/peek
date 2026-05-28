use crate::model::{DefContent, DefKind};
use crate::parser::{
    LanguageParser, MatchMode, build_scope, extract_signature_to_body, first_line_of_node,
    flatten_bytes, line_range, node_text, node_text_ref, normalize_signature,
};
use tree_sitter::{Node, Parser};

pub(crate) const LANGUAGE: &str = "bash";
pub(crate) const EXTENSIONS: &[&str] = &["sh", "bash", "bats"];
pub(crate) const ALIASES: &[&str] = &["shell", "sh"];

pub struct BashParser;

impl LanguageParser for BashParser {
    fn language(&self) -> &'static str {
        LANGUAGE
    }

    fn extensions(&self) -> &'static [&'static str] {
        EXTENSIONS
    }

    fn supported_kinds(&self) -> &'static [DefKind] {
        &[DefKind::Function, DefKind::Const, DefKind::Var]
    }

    impl_init_parser!(tree_sitter_bash::LANGUAGE, "Bash");

    impl_extract_with!(collect_definitions, scope: "");
}

/// Handle a function_definition node: extract the function name and recurse into body.
fn handle_function(
    node: Node,
    source: &str,
    mode: &MatchMode,
    kinds: &[DefKind],
    results: &mut Vec<DefContent>,
    scope: &str,
) {
    let name_node = match node.child_by_field_name("name") {
        Some(n) => n,
        None => return,
    };
    let name_ref = node_text_ref(name_node, source);

    let own_scope = build_scope(scope, "::", name_ref);

    // Extract this function if Function kind is requested and name matches
    if kinds.contains(&DefKind::Function) && mode.matches_ident(name_ref) {
        let signature = extract_signature_to_body(node, source);
        let start_row = node.start_position().row + 1;
        let [start, end] = line_range(start_row, node);

        results.push(DefContent {
            kind: DefKind::Function,
            lines: [start, end],
            signature,
            scope: own_scope.clone(),
        });
    }

    // Always recurse into body to discover nested functions/consts
    if let Some(body) = node.child_by_field_name("body") {
        recurse_children(body, source, mode, kinds, results, &own_scope);
    }
}

/// Handle a declaration_command node that represents a const (readonly or declare -r).
fn handle_const(
    node: Node,
    source: &str,
    mode: &MatchMode,
    kinds: &[DefKind],
    results: &mut Vec<DefContent>,
    scope: &str,
) {
    if !kinds.contains(&DefKind::Const) {
        return;
    }

    // Check if this is a readonly or declare -r
    let mut cursor = node.walk();
    let children: Vec<Node> = node.children(&mut cursor).collect();

    // First anonymous child should be "readonly" or "declare"
    let first_text = children
        .iter()
        .find(|c| !c.is_named())
        .map(|c| node_text(*c, source))
        .unwrap_or_default();

    let is_readonly_cmd = first_text == "readonly";
    let needs_r_flag = first_text == "declare" || first_text == "typeset" || first_text == "local";
    let has_r_flag = needs_r_flag
        && children.iter().any(|c| {
            c.is_named() && c.kind() == "word" && {
                let t = node_text(*c, source);
                t.starts_with('-') && t.contains('r')
            }
        });

    if !is_readonly_cmd && !has_r_flag {
        return;
    }

    // Extract all variable names from this declaration
    for child in &children {
        match child.kind() {
            "variable_name" => {
                // No assignment, just the variable name
                let name_ref = node_text_ref(*child, source);
                if mode.matches_ident(name_ref) {
                    let name = name_ref.to_string();
                    let own_scope = build_scope(scope, "::", &name);
                    let sig = first_line_of_node(node, source);
                    let signature = normalize_signature(&sig);
                    let start_row = node.start_position().row + 1;
                    let [start, end] = line_range(start_row, node);

                    results.push(DefContent {
                        kind: DefKind::Const,
                        lines: [start, end],
                        signature,
                        scope: own_scope,
                    });
                }
            }
            "variable_assignment" => {
                // Has assignment, get name from the name field
                if let Some(var_name_node) = child.child_by_field_name("name") {
                    let name_ref = node_text_ref(var_name_node, source);
                    if mode.matches_ident(name_ref) {
                        let name = name_ref.to_string();
                        let own_scope = build_scope(scope, "::", &name);
                        // Truncate signature to the = sign position
                        let mut inner_cursor = child.walk();
                        let eq_byte = child
                            .children(&mut inner_cursor)
                            .find(|c| c.kind() == "=")
                            .map(|c| c.start_byte())
                            .unwrap_or_else(|| node.end_byte());

                        let sig = flatten_bytes(node.start_byte(), eq_byte, source)
                            .unwrap_or_else(|| first_line_of_node(node, source));
                        let signature = normalize_signature(&sig);
                        let start_row = node.start_position().row + 1;
                        let [start, end] = line_range(start_row, node);

                        results.push(DefContent {
                            kind: DefKind::Const,
                            lines: [start, end],
                            signature,
                            scope: own_scope,
                        });
                    }
                }
            }
            _ => {}
        }
    }
}

/// Handle a variable_assignment node: extract as Var kind.
/// Only extracts top-level variable assignments (not inside function bodies).
fn handle_var(
    node: Node,
    source: &str,
    mode: &MatchMode,
    kinds: &[DefKind],
    results: &mut Vec<DefContent>,
    scope: &str,
) {
    if !kinds.contains(&DefKind::Var) {
        return;
    }

    // Only top-level variable assignments (not inside function bodies)
    if !scope.is_empty() {
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
    let own_scope = build_scope(scope, "::", &name);
    // Truncate signature to the = sign
    let mut cursor = node.walk();
    let eq_byte = node
        .children(&mut cursor)
        .find(|c| c.kind() == "=")
        .map(|c| c.start_byte())
        .unwrap_or_else(|| node.end_byte());
    let sig = flatten_bytes(node.start_byte(), eq_byte, source)
        .unwrap_or_else(|| first_line_of_node(node, source));
    let signature = normalize_signature(&sig);
    let start_row = node.start_position().row + 1;
    let [start, end] = line_range(start_row, node);

    results.push(DefContent {
        kind: DefKind::Var,
        lines: [start, end],
        signature,
        scope: own_scope,
    });
}

/// Recursively walk the AST, dispatching to type-specific handlers.
fn collect_definitions(
    node: Node,
    source: &str,
    mode: &MatchMode,
    kinds: &[DefKind],
    results: &mut Vec<DefContent>,
    scope: &str,
) {
    match node.kind() {
        "function_definition" => {
            handle_function(node, source, mode, kinds, results, scope);
        }
        "declaration_command" => {
            handle_const(node, source, mode, kinds, results, scope);
        }
        "variable_assignment" => {
            handle_var(node, source, mode, kinds, results, scope);
        }
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
