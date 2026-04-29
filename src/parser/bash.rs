use crate::model::{DefContent, DefKind};
use crate::parser::{
    LanguageParser, MatchMode, build_scope, extract_signature_to_body, first_line_of_node,
    flatten_bytes, line_range, node_text, node_text_ref, normalize_signature,
};
use tree_sitter::{Node, Parser};

pub struct BashParser;

impl LanguageParser for BashParser {
    fn language(&self) -> &'static str {
        "bash"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &[".sh", ".bash"]
    }

    fn supported_kinds(&self) -> &'static [DefKind] {
        &[DefKind::Function, DefKind::Const]
    }

    fn scope_separators(&self) -> &'static [&'static str] {
        &["::"]
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

    let is_readonly_cmd = first_text == "readonly" || first_text == "typeset";
    let is_declare_r = first_text == "declare"
        && children.iter().any(|c| {
            c.is_named() && c.kind() == "word" && {
                let t = node_text(*c, source);
                t.starts_with('-') && t.contains('r')
            }
        });

    if !is_readonly_cmd && !is_declare_r {
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

    #[test]
    fn test_language() {
        let parser = BashParser;
        assert_eq!(parser.language(), "bash");
    }

    #[test]
    fn test_extensions() {
        let parser = BashParser;
        assert_eq!(parser.extensions(), &[".sh", ".bash"]);
    }

    #[test]
    fn test_supported_kinds() {
        let parser = BashParser;
        let kinds = parser.supported_kinds();
        assert!(kinds.contains(&DefKind::Function));
        assert!(kinds.contains(&DefKind::Const));
        assert_eq!(kinds.len(), 2);
    }

    // === Edge case / handler tests ===

    #[test]
    fn test_no_extraction_of_local() {
        let source = r#"
function my_func {
    local x=10
}
"#;
        let defs = extract_definitions(&BashParser, "x", &[DefKind::Const], source);
        assert!(
            defs.is_empty(),
            "local variables should not be treated as const"
        );
    }

    #[test]
    fn test_no_extraction_of_declare_without_r() {
        let source = r#"
declare NORMAL_VAR="mutable"
"#;
        let defs = extract_definitions(&BashParser, "NORMAL_VAR", &[DefKind::Const], source);
        assert!(
            defs.is_empty(),
            "declare without -r should not be treated as const"
        );
    }

    #[test]
    fn test_kind_filter_only_functions() {
        let source = r#"
readonly MY_CONST=42
function my_func {
    echo "hi"
}
"#;
        // Only request Function kind -- const should not appear
        let defs = extract_definitions(&BashParser, "MY_CONST", &[DefKind::Function], source);
        assert!(defs.is_empty());
    }

    #[test]
    fn test_kind_filter_only_const() {
        let source = r#"
function my_func {
    echo "hi"
}
"#;
        // Only request Const kind -- function should not appear
        let defs = extract_definitions(&BashParser, "my_func", &[DefKind::Const], source);
        assert!(defs.is_empty());
    }

    #[test]
    fn test_empty_source() {
        let source = "";
        let defs = extract_definitions(&BashParser, "anything", &[DefKind::Function], source);
        assert!(defs.is_empty());
    }

    #[test]
    fn test_no_matching_name() {
        let source = r#"
function build_project {
    echo "building"
}
"#;
        let defs = extract_definitions(&BashParser, "nonexistent", &[DefKind::Function], source);
        assert!(defs.is_empty());
    }

    // === Bug verification: declare -rx not recognized as const ===

    /// Verify that `declare -rx` (readonly + export) is recognized as a const.
    /// BUG: The parser only checks for exactly "-r" in the options, but `declare -rx`
    /// produces a word node with text "-rx" in the AST. This means compound options
    /// containing -r (e.g., -rx, -rg, -rxi) are silently ignored.
    #[test]
    fn test_declare_rx_recognized_as_const() {
        let source = r#"
declare -rx GLOBAL_API_KEY="secret123"
"#;
        let defs = extract_definitions(&BashParser, "GLOBAL_API_KEY", &[DefKind::Const], source);
        assert_eq!(
            defs.len(),
            1,
            "declare -rx should be recognized as const (readonly + export is still readonly)"
        );
        assert_eq!(defs[0].kind, DefKind::Const);
        assert_eq!(defs[0].scope, "GLOBAL_API_KEY");
    }

    /// Verify that `declare -rg` (readonly + global) is recognized as a const.
    #[test]
    fn test_declare_rg_recognized_as_const() {
        let source = r#"
declare -rg GLOBAL_CONFIG="production"
"#;
        let defs = extract_definitions(&BashParser, "GLOBAL_CONFIG", &[DefKind::Const], source);
        assert_eq!(
            defs.len(),
            1,
            "declare -rg should be recognized as const (readonly + global is still readonly)"
        );
        assert_eq!(defs[0].kind, DefKind::Const);
    }

    /// Verify that `typeset -r` (synonym for declare -r) is recognized as a const.
    #[test]
    fn test_typeset_r_recognized_as_const() {
        let source = r#"
typeset -r TYPESET_CONST="typeset_value"
"#;
        let defs = extract_definitions(&BashParser, "TYPESET_CONST", &[DefKind::Const], source);
        assert_eq!(
            defs.len(),
            1,
            "typeset -r should be recognized as const (typeset is a synonym for declare)"
        );
        assert_eq!(defs[0].kind, DefKind::Const);
        assert_eq!(defs[0].scope, "TYPESET_CONST");
    }
}
