use crate::model::{DefContent, DefKind};
use crate::parser::{
    LanguageParser, MatchMode, build_scope, classify_metamethod, extract_dotted_name,
    first_child_by_kind, first_line_of_node, flatten_bytes, line_range, node_text, node_text_ref,
    normalize_signature,
};
use tree_sitter::{Node, Parser};

pub(crate) const LANGUAGE: &str = "lua";
pub(crate) const EXTENSIONS: &[&str] = &["lua", "nse", "rockspec"];
pub(crate) const ALIASES: &[&str] = &[];

pub struct LuaParser;

impl LanguageParser for LuaParser {
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
            DefKind::Const,
            DefKind::Var,
            DefKind::Field,
            DefKind::Operator,
            DefKind::Getter,
            DefKind::Setter,
            DefKind::Destructor,
        ]
    }

    impl_init_parser!(tree_sitter_lua::LANGUAGE, "Lua");

    impl_extract_with!(collect_definitions, scope: "");
}

/// Extract function signature using `parameters.end_byte()` as boundary.
///
/// tree-sitter-lua places body-leading comments as direct children of
/// `function_declaration` between `parameters` and `body`. Using
/// `body.start_byte()` (like the generic `extract_signature_to_body`) includes
/// these stray comments. Using `parameters.end_byte()` correctly excludes them.
fn extract_lua_signature(node: Node, source: &str) -> String {
    let end_byte = node
        .child_by_field_name("parameters")
        .map(|p| p.end_byte())
        .or_else(|| node.child_by_field_name("body").map(|b| b.start_byte()))
        .unwrap_or_else(|| node.end_byte());
    let sig = flatten_bytes(node.start_byte(), end_byte, source)
        .unwrap_or_else(|| first_line_of_node(node, source));
    normalize_signature(&sig)
}

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

    let (full_path, final_name) = match extract_dotted_name(name_node, source) {
        Some(pair) => pair,
        None => return,
    };

    let own_scope = build_scope(scope, ".", &full_path);

    // Determine kind: metamethod overrides take priority, then colon vs dot syntax
    let def_kind = classify_metamethod(&final_name).unwrap_or_else(|| {
        if name_node.kind() == "method_index_expression" {
            DefKind::Method
        } else {
            DefKind::Function
        }
    });

    if kinds.contains(&def_kind) && mode.matches_ident(&final_name) {
        let signature = extract_lua_signature(node, source);
        let start_row = node.start_position().row + 1;
        let [start, end] = line_range(start_row, node);

        results.push(DefContent {
            kind: def_kind,
            lines: [start, end],
            signature,
            scope: own_scope.clone(),
        });
    }

    // Always recurse into body to discover nested functions
    if let Some(body) = node.child_by_field_name("body") {
        recurse_children(body, source, mode, kinds, results, &own_scope);
    }
}

/// Handle `variable_declaration` (local variables): `local x = 1`, `local x`, `local x <const> = 1`
///
/// Only extracts when at top-level (scope is empty) and variable_list contains a single identifier.
/// Checks for `<const>` attribute first — Const takes priority over Var.
fn handle_variable_declaration(
    node: Node,
    source: &str,
    mode: &MatchMode,
    kinds: &[DefKind],
    results: &mut Vec<DefContent>,
    scope: &str,
) {
    if !scope.is_empty()
        || !kinds.contains(&DefKind::Var)
            && !kinds.contains(&DefKind::Const)
            && !kinds.contains(&DefKind::Function)
    {
        return;
    }

    // variable_declaration has two forms:
    // Form A (no assignment): variable_declaration > variable_list
    // Form B (with assignment): variable_declaration > assignment_statement > variable_list
    let var_list = if let Some(vl) = first_child_by_kind(node, "variable_list") {
        vl
    } else if let Some(assignment) = first_child_by_kind(node, "assignment_statement") {
        match first_child_by_kind(assignment, "variable_list") {
            Some(vl) => vl,
            None => return,
        }
    } else {
        return;
    };

    extract_var_or_const(&var_list, node, source, mode, kinds, results);
}

/// Handle top-level `assignment_statement` (global assignment): `x = 1`
///
/// Only extracts when at top-level and variable_list contains a single identifier.
fn handle_assignment_statement(
    node: Node,
    source: &str,
    mode: &MatchMode,
    kinds: &[DefKind],
    results: &mut Vec<DefContent>,
    scope: &str,
) {
    if !scope.is_empty() || !kinds.contains(&DefKind::Var) && !kinds.contains(&DefKind::Function) {
        return;
    }

    let var_list = match first_child_by_kind(node, "variable_list") {
        Some(vl) => vl,
        None => return,
    };

    extract_var_or_const(&var_list, node, source, mode, kinds, results);
}

/// Extract Var, Const, or Function from a variable_list node.
///
/// Const detection: checks if variable_list has an `attribute` child whose identifier is "const".
/// Function detection: checks if expression_list value is a `function_definition`.
/// Only extracts when variable_list contains exactly one plain identifier (no dot/bracket index).
fn extract_var_or_const(
    var_list: &Node,
    parent_node: Node,
    source: &str,
    mode: &MatchMode,
    kinds: &[DefKind],
    results: &mut Vec<DefContent>,
) {
    // Collect all name children and check for attribute
    let mut names: Vec<Node> = Vec::new();
    let mut is_const = false;
    let mut cursor = var_list.walk();
    for child in var_list.children_by_field_name("name", &mut cursor) {
        names.push(child);
    }
    // Check for attribute (e.g., <const>)
    if let Some(attr_node) = var_list.child_by_field_name("attribute") {
        if let Some(attr_ident) = first_child_by_kind(attr_node, "identifier") {
            if node_text_ref(attr_ident, source) == "const" {
                is_const = true;
            }
        }
    }

    // Only extract single-identifier variable lists
    if names.len() != 1 {
        return;
    }
    let name_node = names[0];
    if name_node.kind() != "identifier" {
        return;
    }

    let name_ref = node_text_ref(name_node, source);
    if !mode.matches_ident(name_ref) {
        return;
    }

    let def_kind = if is_const {
        DefKind::Const
    } else if has_function_definition_value(parent_node) {
        DefKind::Function
    } else {
        DefKind::Var
    };

    if !kinds.contains(&def_kind) {
        return;
    }

    let name = name_ref.to_string();
    let signature = first_line_of_node(parent_node, source);
    let start_row = parent_node.start_position().row + 1;
    let [start, end] = line_range(start_row, parent_node);

    results.push(DefContent {
        kind: def_kind,
        lines: [start, end],
        signature,
        scope: name,
    });
}

/// Check if a `variable_declaration` or `assignment_statement` node has a
/// `function_definition` as the sole value in its expression_list.
fn has_function_definition_value(parent_node: Node) -> bool {
    let expr_list = first_child_by_kind(parent_node, "expression_list").or_else(|| {
        first_child_by_kind(parent_node, "assignment_statement")
            .and_then(|a| first_child_by_kind(a, "expression_list"))
    });
    let Some(el) = expr_list else {
        return false;
    };
    let mut cursor = el.walk();
    let children: Vec<Node> = el.children(&mut cursor).collect();
    children.len() == 1 && children[0].kind() == "function_definition"
}

/// Handle `table_constructor`: extract named fields.
///
/// - function_definition value → Method (with body recursion for nested functions)
/// - Other values → Field
/// - Positional fields (no identifier key) are skipped.
fn handle_table_constructor(
    node: Node,
    source: &str,
    mode: &MatchMode,
    kinds: &[DefKind],
    results: &mut Vec<DefContent>,
    scope: &str,
) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() != "field" {
            continue;
        }
        let name_node = match child.child_by_field_name("name") {
            Some(n) if n.kind() == "identifier" => n,
            _ => continue,
        };

        let field_name = node_text(name_node, source);
        let own_scope = build_scope(scope, ".", &field_name);
        let value_node = child.child_by_field_name("value");

        // Always recurse into nested table constructors regardless of name/kind match
        if let Some(value) = value_node {
            if value.kind() == "table_constructor" {
                handle_table_constructor(value, source, mode, kinds, results, &own_scope);
            }
        }

        if !mode.matches_ident(&field_name) {
            continue;
        }

        let def_kind = match value_node.as_ref().map(|v| v.kind()) {
            Some("function_definition") => {
                classify_metamethod(&field_name).unwrap_or(DefKind::Method)
            }
            _ => DefKind::Field,
        };

        if !kinds.contains(&def_kind) {
            continue;
        }

        let signature = first_line_of_node(child, source);
        let start_row = child.start_position().row + 1;
        let [start, end] = line_range(start_row, child);

        results.push(DefContent {
            kind: def_kind,
            lines: [start, end],
            signature,
            scope: own_scope.clone(),
        });

        // Recurse into function bodies for nested definitions
        if let Some(value) = value_node {
            if value.kind() == "function_definition" {
                if let Some(body) = value.child_by_field_name("body") {
                    recurse_children(body, source, mode, kinds, results, &own_scope);
                }
            }
        }
    }
}

fn collect_definitions(
    node: Node,
    source: &str,
    mode: &MatchMode,
    kinds: &[DefKind],
    results: &mut Vec<DefContent>,
    scope: &str,
) {
    match node.kind() {
        "function_declaration" => {
            handle_function(node, source, mode, kinds, results, scope);
        }
        "table_constructor" => {
            handle_table_constructor(node, source, mode, kinds, results, scope);
        }
        "variable_declaration" => {
            handle_variable_declaration(node, source, mode, kinds, results, scope);
            // Recurse into expression_list to find table constructors and nested definitions,
            // but skip assignment_statement to avoid double-extracting variables
            if let Some(assignment) = first_child_by_kind(node, "assignment_statement") {
                if let Some(expr_list) = first_child_by_kind(assignment, "expression_list") {
                    recurse_children(expr_list, source, mode, kinds, results, scope);
                }
            }
        }
        "assignment_statement" => {
            handle_assignment_statement(node, source, mode, kinds, results, scope);
            recurse_children(node, source, mode, kinds, results, scope);
        }
        _ => {
            recurse_children(node, source, mode, kinds, results, scope);
        }
    }
}

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
