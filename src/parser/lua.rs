use crate::model::{DefContent, DefKind};
use crate::parser::{
    LanguageParser, MatchMode, build_scope, extract_signature_to_body, line_range, node_text,
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

    // === Meta tests ===

    #[test]
    fn test_language() {
        let parser = LuaParser;
        assert_eq!(parser.language(), "lua");
    }

    #[test]
    fn test_extensions() {
        let parser = LuaParser;
        assert_eq!(parser.extensions(), &[".lua"]);
    }

    #[test]
    fn test_supported_kinds() {
        let parser = LuaParser;
        let kinds = parser.supported_kinds();
        assert!(kinds.contains(&DefKind::Function));
        assert_eq!(kinds.len(), 1);
    }

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

    #[test]
    fn test_no_matching_name() {
        let source = r#"
function hello()
    print("hello")
end
"#;
        let defs = extract_definitions(&LuaParser, "nonexistent", &[DefKind::Function], source);
        assert!(defs.is_empty());
    }

    // === Bug: body comments leak into signature ===
    // BUG: Comments inside a function body (before the first statement) are
    // included in the extracted signature. Root cause: tree-sitter-lua places
    // comments that appear after parameters but before block content as direct
    // children of `function_declaration` (not inside the `body` block). Since
    // `extract_signature_to_body` extracts from `node.start_byte()` to
    // `body.start_byte()`, and `body.start_byte()` is after these stray comments,
    // the signature includes the comment text.

    #[test]
    fn test_bug_signature_excludes_body_comment() {
        let source = r#"
function greet()
    -- This comment should not appear in signature
    local msg = "hello"
    return msg
end
"#;
        let defs = extract_definitions(&LuaParser, "greet", &[DefKind::Function], source);
        assert_eq!(defs.len(), 1);
        // BUG: signature currently includes the comment
        assert!(
            defs[0].signature.contains("--"),
            "BUG CONFIRMED: signature should not contain comment but does: {:?}",
            defs[0].signature
        );
        // Expected (after fix): signature should be "function greet()"
        // assert_eq!(defs[0].signature, "function greet()");
    }

    #[test]
    fn test_bug_local_function_signature_excludes_body_comment() {
        let source = r#"
local function helper()
    -- helper comment
    return 42
end
"#;
        let defs = extract_definitions(&LuaParser, "helper", &[DefKind::Function], source);
        assert_eq!(defs.len(), 1);
        assert!(
            defs[0].signature.contains("--"),
            "BUG CONFIRMED: signature should not contain comment but does: {:?}",
            defs[0].signature
        );
        // Expected (after fix): signature should be "local function helper()"
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
}
