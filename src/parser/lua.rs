use crate::model::{DefContent, DefKind};
use crate::parser::{
    LanguageParser, MatchMode, build_scope, first_line_of_node, flatten_bytes, line_range,
    node_text, normalize_signature,
};
use tree_sitter::{Node, Parser};

pub struct LuaParser;

impl LanguageParser for LuaParser {
    fn language(&self) -> &'static str {
        "lua"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &[".lua"]
    }

    fn supported_kinds(&self) -> &'static [DefKind] {
        &[DefKind::Function]
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

fn extract_name(name_node: Node, source: &str) -> Option<(String, String)> {
    match name_node.kind() {
        "identifier" => {
            let text = node_text(name_node, source);
            Some((text.clone(), text))
        }
        "dot_index_expression" | "method_index_expression" => {
            let table = name_node.child_by_field_name("table")?;
            let child = name_node
                .child_by_field_name("field")
                .or_else(|| name_node.child_by_field_name("method"))?;
            let (table_path, _) = extract_name(table, source)?;
            let child_text = node_text(child, source);
            let full_path = format!("{}.{}", table_path, child_text);
            Some((full_path, child_text))
        }
        _ => None,
    }
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

    let (full_path, final_name) = match extract_name(name_node, source) {
        Some(pair) => pair,
        None => return,
    };

    let own_scope = build_scope(scope, ".", &full_path);

    if kinds.contains(&DefKind::Function) && mode.matches_ident(&final_name) {
        let signature = extract_lua_signature(node, source);
        let start_row = node.start_position().row + 1;
        let [start, end] = line_range(start_row, node);

        results.push(DefContent {
            kind: DefKind::Function,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::extract_definitions;

    // === Edge case tests ===

    #[test]
    fn test_kind_filter_rejects_class() {
        let source = r#"
function my_func()
    return 1
end
"#;
        let defs = extract_definitions(&LuaParser, "my_func", &[DefKind::Class], source);
        assert!(
            defs.is_empty(),
            "Function should not match Class kind filter"
        );
    }

    #[test]
    fn test_no_match_for_local_variable() {
        let source = r#"
local x = 10
local y = function() return 1 end
"#;
        let defs = extract_definitions(&LuaParser, "x", &[DefKind::Function], source);
        assert!(defs.is_empty(), "Local variables should not be extracted");
        let defs = extract_definitions(&LuaParser, "y", &[DefKind::Function], source);
        assert!(
            defs.is_empty(),
            "Anonymous function assignments should not be extracted"
        );
    }

    #[test]
    fn test_empty_source() {
        let source = "";
        let defs = extract_definitions(&LuaParser, "anything", &[DefKind::Function], source);
        assert!(defs.is_empty());
    }

    // === Signature: body comments excluded ===
    // tree-sitter-lua places comments between parameters and body as direct
    // children of function_declaration (not inside the body block).
    // The Lua-specific signature extractor uses parameters.end_byte() to
    // avoid including these stray comments.

    #[test]
    fn test_signature_excludes_body_comment() {
        let source = r#"
function greet()
    -- This comment should not appear in signature
    local msg = "hello"
    return msg
end
"#;
        let defs = extract_definitions(&LuaParser, "greet", &[DefKind::Function], source);
        assert_eq!(defs.len(), 1);
        assert_eq!(defs[0].signature, "function greet()");
    }

    #[test]
    fn test_local_function_signature_excludes_body_comment() {
        let source = r#"
local function helper()
    -- helper comment
    return 42
end
"#;
        let defs = extract_definitions(&LuaParser, "helper", &[DefKind::Function], source);
        assert_eq!(defs.len(), 1);
        assert_eq!(defs[0].signature, "local function helper()");
    }

    #[test]
    fn test_no_comment_function_signature_is_clean() {
        let source = r#"
function clean_func()
    return "clean"
end
"#;
        let defs = extract_definitions(&LuaParser, "clean_func", &[DefKind::Function], source);
        assert_eq!(defs.len(), 1);
        assert_eq!(defs[0].signature, "function clean_func()");
    }

    #[test]
    fn test_function_with_params_signature_excludes_body_comment() {
        let source = r#"
function add(a, b)
    -- compute sum
    return a + b
end
"#;
        let defs = extract_definitions(&LuaParser, "add", &[DefKind::Function], source);
        assert_eq!(defs.len(), 1);
        assert_eq!(defs[0].signature, "function add(a, b)");
    }

    #[test]
    fn test_method_signature_excludes_body_comment() {
        let source = r#"
function MyClass:greet()
    -- method body comment
    return "hello"
end
"#;
        let defs = extract_definitions(&LuaParser, "greet", &[DefKind::Function], source);
        assert_eq!(defs.len(), 1);
        assert_eq!(defs[0].signature, "function MyClass:greet()");
    }
}
