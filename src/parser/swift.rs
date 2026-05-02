use crate::model::{DefContent, DefKind};
use crate::parser::{
    LanguageParser, MatchMode, build_scope, first_child_by_kind, first_line_of_node, flatten_bytes,
    line_range, node_text, node_text_ref, normalize_signature,
};
use tree_sitter::{Node, Parser};

pub struct SwiftParser;

impl LanguageParser for SwiftParser {
    fn language(&self) -> &'static str {
        "swift"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &[".swift", ".swiftinterface"]
    }

    fn supported_kinds(&self) -> &'static [DefKind] {
        &[
            DefKind::Function,
            DefKind::Class,
            DefKind::Struct,
            DefKind::Enum,
            DefKind::Protocol,
            DefKind::Type,
            DefKind::Const,
            DefKind::Actor,
            DefKind::Extension,
        ]
    }

    impl_init_parser!(tree_sitter_swift::LANGUAGE, "Swift");

    impl_extract_with!(collect_definitions, scope: "");
}

/// Determine the DefKind from a `class_declaration` node's `declaration_kind` field.
///
/// Tree-sitter-swift uses `class_declaration` for class, struct, enum, actor,
/// and extension -- distinguished by the `declaration_kind` anonymous child text.
fn classify_class_declaration(node: Node, source: &str) -> DefKind {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if !child.is_named() {
            if let Ok(text) = child.utf8_text(source.as_bytes()) {
                match text.trim() {
                    "struct" => return DefKind::Struct,
                    "enum" => return DefKind::Enum,
                    "actor" => return DefKind::Actor,
                    "extension" => return DefKind::Extension,
                    "class" => return DefKind::Class,
                    _ => continue,
                }
            }
        }
    }
    DefKind::Class
}

/// Find the body node of a class_declaration.
///
/// Body node types vary by declaration kind:
/// - class/struct/actor/extension use `class_body`
/// - enum uses `enum_class_body`
fn find_body_node(node: Node) -> Option<Node> {
    node.child_by_field_name("body").or_else(|| {
        let mut cursor = node.walk();
        node.children(&mut cursor)
            .find(|c| c.kind() == "class_body" || c.kind() == "enum_class_body")
    })
}

/// Handle a class_declaration node: extract the definition and recurse into body.
///
/// Covers Class, Struct, Enum, Actor, and Extension (after classification).
/// For extension, the name is extracted from `user_type > type_identifier`.
fn handle_class_declaration(
    node: Node,
    source: &str,
    mode: &MatchMode,
    def_kind: DefKind,
    kinds: &[DefKind],
    results: &mut Vec<DefContent>,
    scope: &str,
) {
    let body = find_body_node(node);

    let own_scope = build_scope_from_node(node, source, scope, def_kind);
    if kinds.contains(&def_kind) {
        if let Some(name_node) = node.child_by_field_name("name") {
            let name_ref = if def_kind == DefKind::Extension && name_node.kind() == "user_type" {
                first_child_by_kind(name_node, "type_identifier")
                    .or_else(|| first_child_by_kind(name_node, "simple_identifier"))
            } else {
                Some(name_node)
            };

            if let Some(name_node) = name_ref {
                let name_ref = node_text_ref(name_node, source);

                if mode.matches_ident(name_ref) {
                    let sig = match body {
                        Some(b) => flatten_bytes(node.start_byte(), b.start_byte(), source)
                            .unwrap_or_else(|| first_line_of_node(node, source)),
                        None => first_line_of_node(node, source),
                    };
                    let signature = normalize_signature(&sig);
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
        }
    }

    // Always recurse into body to discover nested types
    if let Some(body) = body {
        recurse_children(body, source, mode, kinds, results, &own_scope);
    }
}

/// Extract the name text from a class_declaration node.
///
/// For extension, name is in `user_type > type_identifier` (accessed via the `name` field).
/// For other types, name is a direct `type_identifier` via the `name` field.
fn extract_declaration_name(node: Node, source: &str, def_kind: DefKind) -> Option<String> {
    let name_node = node.child_by_field_name("name")?;
    if def_kind == DefKind::Extension {
        // Extension name is `user_type(type_identifier)` -- get the inner type_identifier
        if name_node.kind() == "user_type" {
            let inner = first_child_by_kind(name_node, "type_identifier")
                .or_else(|| first_child_by_kind(name_node, "simple_identifier"));
            return inner.map(|n| node_text(n, source));
        }
    }
    Some(node_text(name_node, source))
}

/// Handle a protocol_declaration node.
fn handle_protocol(
    node: Node,
    source: &str,
    mode: &MatchMode,
    kinds: &[DefKind],
    results: &mut Vec<DefContent>,
    scope: &str,
) {
    let body = first_child_by_kind(node, "protocol_body");
    let own_scope = build_scope_from_node(node, source, scope, DefKind::Protocol);

    if kinds.contains(&DefKind::Protocol) {
        let name_node = match node.child_by_field_name("name") {
            Some(n) => n,
            None => return,
        };
        let name_ref = node_text_ref(name_node, source);

        if mode.matches_ident(name_ref) {
            let sig = match body {
                Some(b) => flatten_bytes(node.start_byte(), b.start_byte(), source)
                    .unwrap_or_else(|| first_line_of_node(node, source)),
                None => first_line_of_node(node, source),
            };
            let signature = normalize_signature(&sig);
            let start_row = node.start_position().row + 1;
            let [start, end] = line_range(start_row, node);

            results.push(DefContent {
                kind: DefKind::Protocol,
                lines: [start, end],
                signature,
                scope: own_scope.clone(),
            });
        }
    }

    // Always recurse into body
    if let Some(body) = body {
        recurse_children(body, source, mode, kinds, results, &own_scope);
    }
}

/// Handle a function_declaration node.
fn handle_function(
    node: Node,
    source: &str,
    mode: &MatchMode,
    kinds: &[DefKind],
    results: &mut Vec<DefContent>,
    scope: &str,
) {
    if !kinds.contains(&DefKind::Function) {
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

    let own_scope = build_scope(scope, ".", name_ref);
    let raw_sig = if let Some(body) = first_child_by_kind(node, "function_body") {
        flatten_bytes(node.start_byte(), body.start_byte(), source)
            .unwrap_or_else(|| first_line_of_node(node, source))
    } else {
        // Protocol function declaration -- no body
        first_line_of_node(node, source)
    };
    let signature = normalize_signature(&raw_sig);
    let start_row = node.start_position().row + 1;
    let [start, end] = line_range(start_row, node);

    results.push(DefContent {
        kind: DefKind::Function,
        lines: [start, end],
        signature,
        scope: own_scope,
    });
}

/// Handle a typealias_declaration node.
fn handle_typealias(
    node: Node,
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

    let own_scope = build_scope(scope, ".", name_ref);
    let sig = first_line_of_node(node, source);
    let signature = normalize_signature(&sig);
    let start_row = node.start_position().row + 1;
    let [start, end] = line_range(start_row, node);

    results.push(DefContent {
        kind: DefKind::Type,
        lines: [start, end],
        signature,
        scope: own_scope,
    });
}

/// Handle a property_declaration node, extracting only `let` constants.
///
/// Tree-sitter-swift property_declaration structure:
/// - `value_binding_pattern` with `mutability` field ("let" or "var")
/// - `pattern` with `bound_identifier` field containing `simple_identifier`
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

    // Check if this is a let (constant) declaration
    if !is_let_property(node, source) {
        return;
    }

    // Extract name from pattern > bound_identifier > simple_identifier
    let name_ref = extract_property_name_node(node).map(|n| node_text_ref(n, source));

    if let Some(name_ref) = name_ref {
        if !mode.matches_ident(name_ref) {
            return;
        }

        let name = name_ref.to_string();
        let own_scope = if scope.is_empty() {
            name
        } else {
            format!("{}.{}", scope, name)
        };
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

/// Check if a property_declaration uses `let` (constant) binding.
fn is_let_property(node: Node, source: &str) -> bool {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "value_binding_pattern" {
            if let Some(mut_field) = child.child_by_field_name("mutability") {
                if let Ok(text) = mut_field.utf8_text(source.as_bytes()) {
                    return text.trim() == "let";
                }
            }
        }
    }
    false
}

/// Extract property name node (bound_identifier) from a property_declaration node.
fn extract_property_name_node(node: Node) -> Option<Node> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "pattern" {
            if let Some(bound_id) = child.child_by_field_name("bound_identifier") {
                return Some(bound_id);
            }
        }
    }
    None
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
        "class_declaration" => {
            let def_kind = classify_class_declaration(node, source);
            handle_class_declaration(node, source, mode, def_kind, kinds, results, scope);
        }
        "protocol_declaration" => {
            handle_protocol(node, source, mode, kinds, results, scope);
        }
        "function_declaration" => {
            handle_function(node, source, mode, kinds, results, scope);
        }
        "protocol_function_declaration" => {
            handle_function(node, source, mode, kinds, results, scope);
        }
        "typealias_declaration" => {
            handle_typealias(node, source, mode, kinds, results, scope);
        }
        "property_declaration" => {
            handle_const(node, source, mode, kinds, results, scope);
        }
        _ => {
            recurse_children(node, source, mode, kinds, results, scope);
        }
    }
}

/// Build a new scope string by appending the current node's name to the parent scope.
fn build_scope_from_node(
    node: Node,
    source: &str,
    parent_scope: &str,
    def_kind: DefKind,
) -> String {
    let name = extract_declaration_name(node, source, def_kind).unwrap_or_default();
    build_scope(parent_scope, ".", &name)
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

    // --- Edge case tests ---

    #[test]
    fn test_var_not_extracted_as_const() {
        let source = "var count = 0";
        let defs = extract_definitions(&SwiftParser, "count", &[DefKind::Const], source);
        assert!(defs.is_empty());
    }

    #[test]
    fn test_empty_source() {
        let source = "";
        let defs = extract_definitions(&SwiftParser, "anything", &[DefKind::Function], source);
        assert!(defs.is_empty());
    }

    #[test]
    fn test_kind_filter() {
        let source = "class MyClass {}";
        let defs = extract_definitions(&SwiftParser, "MyClass", &[DefKind::Function], source);
        assert!(defs.is_empty());
    }
}
