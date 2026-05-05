use crate::model::{DefContent, DefKind};
use crate::parser::{
    LanguageParser, MatchMode, build_scope, build_scope_from_node, extract_signature_to_body,
    first_line_of_node, flatten_bytes, line_range, node_text_ref, normalize_signature,
};
use tree_sitter::{Node, Parser};

pub struct RubyParser;

impl LanguageParser for RubyParser {
    fn language(&self) -> &'static str {
        "ruby"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &[
            ".rb",
            ".rake",
            ".gemspec",
            ".ru",
            ".rbi",
            ".podspec",
            ".jbuilder",
            ".thor",
            ".rabl",
            ".builder",
            ".god",
        ]
    }

    fn supported_kinds(&self) -> &'static [DefKind] {
        &[
            DefKind::Method,
            DefKind::Class,
            DefKind::Module,
            DefKind::Const,
        ]
    }

    impl_init_parser!(tree_sitter_ruby::LANGUAGE, "Ruby");

    impl_extract_with!(collect_definitions, scope: "");
}

/// Handle a container node (module or class): extract the definition and recurse into body.
fn handle_container(
    node: Node,
    source: &str,
    mode: &MatchMode,
    kinds: &[DefKind],
    results: &mut Vec<DefContent>,
    scope: &str,
    def_kind: DefKind,
) {
    let body = node.child_by_field_name("body");
    let own_scope = build_scope_from_node(node, source, scope, "::");

    if kinds.contains(&def_kind) {
        let name_node = match node.child_by_field_name("name") {
            Some(n) => n,
            None => return,
        };
        let name_ref = node_text_ref(name_node, source);

        if mode.matches_ident(name_ref) {
            let signature = extract_signature_to_body(node, source);
            let start_row = node.start_position().row + 1;
            let [start, end] = line_range(start_row, node);

            results.push(DefContent {
                kind: def_kind,
                lines: [start, end],
                signature,
                scope: own_scope.clone(),
            });
        }
    }

    // Always recurse into body to discover nested types
    if let Some(body) = body {
        recurse_children(body, source, mode, kinds, results, &own_scope);
    }
}

/// Handle a method node (both instance methods and singleton methods).
fn handle_method(
    node: Node,
    source: &str,
    mode: &MatchMode,
    kinds: &[DefKind],
    results: &mut Vec<DefContent>,
    scope: &str,
) {
    if !kinds.contains(&DefKind::Method) {
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

    let own_scope = build_scope(scope, "::", name_ref);
    let signature = extract_signature_to_body(node, source);
    let start_row = node.start_position().row + 1;
    let [start, end] = line_range(start_row, node);

    results.push(DefContent {
        kind: DefKind::Method,
        lines: [start, end],
        signature,
        scope: own_scope,
    });
}

/// Check if an assignment node has a constant on the left side.
fn is_constant_assignment(node: Node) -> bool {
    if let Some(left) = node.child_by_field_name("left") {
        left.kind() == "constant"
    } else {
        false
    }
}

/// Handle a constant assignment node.
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

    let left_node = match node.child_by_field_name("left") {
        Some(n) => n,
        None => return,
    };
    let name_ref = node_text_ref(left_node, source);

    if !mode.matches_ident(name_ref) {
        return;
    }

    let own_scope = build_scope(scope, "::", name_ref);
    // Find the "=" token to truncate signature
    let eq_byte = {
        let mut cursor = node.walk();
        node.children(&mut cursor)
            .find(|c| c.kind() == "=")
            .map(|c| c.start_byte())
            .unwrap_or_else(|| node.end_byte())
    };

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
        "module" => {
            handle_container(node, source, mode, kinds, results, scope, DefKind::Module);
        }
        "class" => {
            handle_container(node, source, mode, kinds, results, scope, DefKind::Class);
        }
        "method" | "singleton_method" => {
            handle_method(node, source, mode, kinds, results, scope);
            // Do not recurse into method body
        }
        "assignment" => {
            if is_constant_assignment(node) {
                handle_const(node, source, mode, kinds, results, scope);
            }
            // Do not recurse into assignment
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

    // === Edge case tests ===

    #[test]
    fn test_no_extraction_of_local_variable() {
        let source = r#"
my_var = 42
"#;
        let parser = RubyParser;
        let mut ts_parser = parser.init_parser();
        let mode = MatchMode::from_user_input("my_var", false, false).unwrap();
        let defs = parser
            .extract_with(&mode, &[DefKind::Const], source, &mut ts_parser)
            .unwrap();
        // my_var is lowercase, should not be parsed as a constant node
        assert!(defs.is_empty());
    }

    #[test]
    fn test_kind_filter_only_functions() {
        let source = r#"
class MyClass
  def my_method
  end
end
"#;
        let parser = RubyParser;
        let mut ts_parser = parser.init_parser();
        let mode = MatchMode::from_user_input("MyClass", false, false).unwrap();
        let defs = parser
            .extract_with(&mode, &[DefKind::Function], source, &mut ts_parser)
            .unwrap();
        assert!(defs.is_empty());
    }

    #[test]
    fn test_empty_source() {
        let source = "";
        let parser = RubyParser;
        let mut ts_parser = parser.init_parser();
        let mode = MatchMode::from_user_input("anything", false, false).unwrap();
        let defs = parser
            .extract_with(&mode, &[DefKind::Function], source, &mut ts_parser)
            .unwrap();
        assert!(defs.is_empty());
    }
}
