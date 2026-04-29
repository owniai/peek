use crate::model::{DefContent, DefKind};
use crate::parser::{
    LanguageParser, MatchMode, build_scope, build_scope_from_node, first_child_by_kind,
    first_line_of_node, flatten_bytes, line_range, node_text, node_text_ref, normalize_signature,
};
use tree_sitter::{Node, Parser};

pub struct CSharpParser;

impl LanguageParser for CSharpParser {
    fn language(&self) -> &'static str {
        "csharp"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &[".cs"]
    }

    fn supported_kinds(&self) -> &'static [DefKind] {
        &[
            DefKind::Class,
            DefKind::Interface,
            DefKind::Enum,
            DefKind::Struct,
            DefKind::Record,
            DefKind::Delegate,
            DefKind::Event,
            DefKind::Function,
            DefKind::Const,
        ]
    }

    impl_init_parser!(tree_sitter_c_sharp::LANGUAGE, "C#");

    fn extract_with(
        &self,
        mode: &MatchMode,
        kinds: &[DefKind],
        source: &str,
        parser: &mut Parser,
    ) -> Result<Vec<DefContent>, ()> {
        let tree = match parser.parse(source, None) {
            Some(tree) => tree,
            None => return Err(()),
        };
        let root = tree.root_node();

        let mut results = Vec::new();

        // Walk root's direct children. When we encounter a file_scoped_namespace_declaration,
        // switch to its scope for all subsequent siblings (C# semantics).
        let mut current_scope: Option<String> = None;
        let mut cursor = root.walk();
        for child in root.children(&mut cursor) {
            match child.kind() {
                "file_scoped_namespace_declaration" => {
                    let ns = extract_namespace_name(child, source);
                    current_scope = Some(ns);
                }
                _ => {
                    let scope_str = current_scope.as_deref().unwrap_or("");
                    collect_definitions(child, source, mode, kinds, &mut results, scope_str);
                }
            }
        }

        Ok(results)
    }
}

/// Extract the full qualified name from a namespace node's name field.
/// Handles both simple identifiers and qualified names (e.g. "MyApp.Models").
fn extract_namespace_name(node: Node, source: &str) -> String {
    let name_node = match node.child_by_field_name("name") {
        Some(n) => n,
        None => return String::new(),
    };
    extract_name_recursive(name_node, source)
}

/// Recursively extract a qualified name: "identifier" or "qualifier.name".
fn extract_name_recursive(node: Node, source: &str) -> String {
    match node.kind() {
        "identifier" => node_text(node, source),
        "qualified_name" => {
            let qualifier = node
                .child_by_field_name("qualifier")
                .map(|n| extract_name_recursive(n, source))
                .unwrap_or_default();
            let name = node
                .child_by_field_name("name")
                .map(|n| extract_name_recursive(n, source))
                .unwrap_or_default();
            if qualifier.is_empty() {
                name
            } else {
                format!("{}.{}", qualifier, name)
            }
        }
        _ => node_text(node, source),
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

/// Recursively walk the AST, dispatching to type-specific handlers.
/// `scope` is the dot-separated context string (e.g. "MyApp.Models.User") built from
/// namespace and ancestor type nodes.
fn collect_definitions(
    node: Node,
    source: &str,
    mode: &MatchMode,
    kinds: &[DefKind],
    results: &mut Vec<DefContent>,
    scope: &str,
) {
    match node.kind() {
        // --- Namespace handling ---
        "namespace_declaration" => {
            let ns_name = extract_namespace_name(node, source);
            let new_scope = build_scope(scope, ".", &ns_name);
            if let Some(body) = first_child_by_kind(node, "declaration_list") {
                recurse_children(body, source, mode, kinds, results, &new_scope);
            }
        }
        "file_scoped_namespace_declaration" => {
            // Handled in extract_with() at the top level; skip here.
        }
        // --- Type definitions ---
        "class_declaration" => {
            let own_scope = build_scope_from_node(node, source, scope, ".");
            if kinds.contains(&DefKind::Class) {
                handle_type_definition(node, source, mode, DefKind::Class, results, &own_scope);
            }
            recurse_into_type_body(node, source, mode, kinds, results, &own_scope);
        }
        "interface_declaration" => {
            let own_scope = build_scope_from_node(node, source, scope, ".");
            if kinds.contains(&DefKind::Interface) {
                handle_type_definition(node, source, mode, DefKind::Interface, results, &own_scope);
            }
            recurse_into_type_body(node, source, mode, kinds, results, &own_scope);
        }
        "struct_declaration" => {
            let own_scope = build_scope_from_node(node, source, scope, ".");
            if kinds.contains(&DefKind::Struct) {
                handle_type_definition(node, source, mode, DefKind::Struct, results, &own_scope);
            }
            recurse_into_type_body(node, source, mode, kinds, results, &own_scope);
        }
        "enum_declaration" => {
            let own_scope = build_scope_from_node(node, source, scope, ".");
            if kinds.contains(&DefKind::Enum) {
                handle_type_definition(node, source, mode, DefKind::Enum, results, &own_scope);
            }
            recurse_into_type_body(node, source, mode, kinds, results, &own_scope);
        }
        "record_declaration" => {
            let own_scope = build_scope_from_node(node, source, scope, ".");
            if kinds.contains(&DefKind::Record) {
                handle_type_definition(node, source, mode, DefKind::Record, results, &own_scope);
            }
            // record may have a body (declaration_list) or end with `;`
            recurse_into_type_body(node, source, mode, kinds, results, &own_scope);
        }
        // --- Method ---
        "method_declaration" | "constructor_declaration" => {
            if kinds.contains(&DefKind::Function) {
                handle_method(node, source, mode, results, scope);
            }
            // Do not recurse into method/constructor body
        }
        // --- Delegate ---
        "delegate_declaration" => {
            if kinds.contains(&DefKind::Delegate) {
                handle_delegate(node, source, mode, results, scope);
            }
        }
        // --- Event ---
        "event_field_declaration" => {
            if kinds.contains(&DefKind::Event) {
                handle_event(node, source, mode, results, scope);
            }
        }
        // --- Const field ---
        "field_declaration" => {
            if kinds.contains(&DefKind::Const) && is_const_field(node) {
                push_const_declarators(node, source, mode, results, scope);
            }
        }
        // --- Skip: not definitions we extract ---
        "using_directive" | "global_attribute" => {}
        // --- Recurse into all other nodes ---
        _ => {
            recurse_children(node, source, mode, kinds, results, scope);
        }
    }
}

/// Recurse into a type node's body (declaration_list), if present.
fn recurse_into_type_body(
    node: Node,
    source: &str,
    mode: &MatchMode,
    kinds: &[DefKind],
    results: &mut Vec<DefContent>,
    scope: &str,
) {
    if let Some(body) = first_child_by_kind(node, "declaration_list") {
        recurse_children(body, source, mode, kinds, results, scope);
    }
    // Also check for enum_member_declaration_list for enum bodies
    if let Some(body) = first_child_by_kind(node, "enum_member_declaration_list") {
        recurse_children(body, source, mode, kinds, results, scope);
    }
}

/// Handle a type-level definition node (class, interface, struct, enum, record).
/// Signature: flatten from node start to body boundary (or `;`), fallback to first line.
fn handle_type_definition(
    node: Node,
    source: &str,
    mode: &MatchMode,
    def_kind: DefKind,
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

    let signature = extract_type_signature(node, source);
    let start_row = node.start_position().row + 1;
    let [start, end] = line_range(start_row, node);

    results.push(DefContent {
        kind: def_kind,
        lines: [start, end],
        signature,
        scope: scope.to_string(),
    });
}

/// Extract a type definition's signature: from node start to the body boundary or `;`.
fn extract_type_signature(node: Node, source: &str) -> String {
    // Try to find body boundary (declaration_list or enum_member_declaration_list)
    let mut end_byte = node.end_byte();
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "declaration_list" | "enum_member_declaration_list" => {
                end_byte = child.start_byte();
                break;
            }
            ";" => {
                end_byte = child.end_byte();
                break;
            }
            _ => {}
        }
    }
    flatten_bytes(node.start_byte(), end_byte, source)
        .map(|s| normalize_signature(&s))
        .unwrap_or_else(|| first_line_of_node(node, source))
}

/// Handle a method_declaration node.
/// Signature: from node start to body boundary, fallback to first line.
fn handle_method(
    node: Node,
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

    let own_scope = build_scope_from_node(node, source, scope, ".");
    let signature = extract_method_signature(node, source);
    let start_row = node.start_position().row + 1;
    let [start, end] = line_range(start_row, node);

    results.push(DefContent {
        kind: DefKind::Function,
        lines: [start, end],
        signature,
        scope: own_scope,
    });
}

/// Extract a method's signature: from node start to body boundary.
fn extract_method_signature(node: Node, source: &str) -> String {
    if let Some(body) = node.child_by_field_name("body") {
        flatten_bytes(node.start_byte(), body.start_byte(), source)
            .map(|s| normalize_signature(&s))
            .unwrap_or_else(|| first_line_of_node(node, source))
    } else {
        first_line_of_node(node, source)
    }
}

/// Handle a delegate_declaration node.
/// Signature: from node start to `;`.
fn handle_delegate(
    node: Node,
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

    let own_scope = build_scope_from_node(node, source, scope, ".");
    let signature = extract_signature_to_semicolon(node, source);
    let start_row = node.start_position().row + 1;
    let [start, end] = line_range(start_row, node);

    results.push(DefContent {
        kind: DefKind::Delegate,
        lines: [start, end],
        signature,
        scope: own_scope,
    });
}

/// Extract a signature by truncating at the first `;` child node.
fn extract_signature_to_semicolon(node: Node, source: &str) -> String {
    let mut end_byte = node.end_byte();
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == ";" {
            end_byte = child.start_byte();
            break;
        }
    }
    flatten_bytes(node.start_byte(), end_byte, source)
        .map(|s| normalize_signature(&s))
        .unwrap_or_else(|| first_line_of_node(node, source))
}

/// Handle an event_field_declaration node.
/// Name extraction: variable_declaration -> variable_declarator -> name -> identifier.
/// Signature: from node start to `;`.
fn handle_event(
    node: Node,
    source: &str,
    mode: &MatchMode,
    results: &mut Vec<DefContent>,
    scope: &str,
) {
    let name_node = extract_event_name_node(node);
    let name_ref = match name_node {
        Some(n) => node_text_ref(n, source),
        None => return,
    };
    if !mode.matches_ident(name_ref) {
        return;
    }

    let own_scope = build_scope(scope, ".", name_ref);
    let signature = extract_signature_to_semicolon(node, source);
    let start_row = node.start_position().row + 1;
    let [start, end] = line_range(start_row, node);

    results.push(DefContent {
        kind: DefKind::Event,
        lines: [start, end],
        signature,
        scope: own_scope,
    });
}

/// Extract event name: variable_declaration -> variable_declarator -> name -> identifier.
fn extract_event_name_node(node: Node) -> Option<Node> {
    let var_decl = first_child_by_kind(node, "variable_declaration")?;
    let declarator = first_child_by_kind(var_decl, "variable_declarator")?;
    declarator.child_by_field_name("name")
}

/// Check if a field_declaration has a `const` modifier.
fn is_const_field(node: Node) -> bool {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "const" {
            return true;
        }
        // Check inside modifier (singular) node — tree-sitter-c-sharp wraps
        // each keyword in an individual `modifier` node.
        if child.kind() == "modifier" {
            let mut mod_cursor = child.walk();
            for mod_child in child.children(&mut mod_cursor) {
                if mod_child.kind() == "const" {
                    return true;
                }
            }
        }
        // Also check modifiers (plural) node for robustness
        if child.kind() == "modifiers" {
            let mut mod_cursor = child.walk();
            for mod_child in child.children(&mut mod_cursor) {
                if mod_child.kind() == "const" {
                    return true;
                }
            }
        }
    }
    false
}

/// Iterate over variable_declarator children of a field_declaration,
/// pushing a Definition for each one whose name matches `mode`.
fn push_const_declarators(
    node: Node,
    source: &str,
    mode: &MatchMode,
    results: &mut Vec<DefContent>,
    scope: &str,
) {
    let sig = extract_const_signature(node, source);
    let start_row = node.start_position().row + 1;
    let [start, end] = line_range(start_row, node);

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() != "variable_declaration" {
            continue;
        }
        let mut vd_cursor = child.walk();
        for vd_child in child.children(&mut vd_cursor) {
            if vd_child.kind() != "variable_declarator" {
                continue;
            }
            let name_node = match vd_child.child_by_field_name("name") {
                Some(n) => n,
                None => continue,
            };
            let name_ref = node_text_ref(name_node, source);
            if !mode.matches_ident(name_ref) {
                continue;
            }

            let own_scope = build_scope(scope, ".", name_ref);
            results.push(DefContent {
                kind: DefKind::Const,
                lines: [start, end],
                signature: sig.clone(),
                scope: own_scope,
            });
        }
    }
}

/// Extract const field signature: from node start to `=` or `;`.
fn extract_const_signature(node: Node, source: &str) -> String {
    let mut end_byte = node.end_byte();
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == ";" {
            end_byte = child.start_byte();
            break;
        }
    }
    // Also try to truncate at `=` within variable_declarator for cleaner signature
    if let Some(var_decl) = first_child_by_kind(node, "variable_declaration") {
        if let Some(declarator) = first_child_by_kind(var_decl, "variable_declarator") {
            let mut decl_cursor = declarator.walk();
            for decl_child in declarator.children(&mut decl_cursor) {
                if decl_child.kind() == "=" {
                    let eq_byte = decl_child.start_byte();
                    if eq_byte < end_byte {
                        end_byte = eq_byte;
                    }
                    break;
                }
            }
        }
    }
    flatten_bytes(node.start_byte(), end_byte, source)
        .map(|s| normalize_signature(&s))
        .unwrap_or_else(|| first_line_of_node(node, source))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::extract_definitions;

    // === Meta tests ===

    #[test]
    fn test_language_returns_csharp() {
        let p = CSharpParser;
        assert_eq!(p.language(), "csharp");
    }

    #[test]
    fn test_extensions_cover_cs() {
        let p = CSharpParser;
        assert!(p.extensions().contains(&".cs"));
        assert_eq!(p.extensions().len(), 1);
    }

    #[test]
    fn test_supported_kinds_nine() {
        let p = CSharpParser;
        let kinds = p.supported_kinds();
        assert!(kinds.contains(&DefKind::Class));
        assert!(kinds.contains(&DefKind::Interface));
        assert!(kinds.contains(&DefKind::Enum));
        assert!(kinds.contains(&DefKind::Struct));
        assert!(kinds.contains(&DefKind::Record));
        assert!(kinds.contains(&DefKind::Delegate));
        assert!(kinds.contains(&DefKind::Event));
        assert!(kinds.contains(&DefKind::Function));
        assert!(kinds.contains(&DefKind::Const));
        assert_eq!(kinds.len(), 9);
    }

    // === Edge case tests ===

    #[test]
    fn test_extract_with_empty_source() {
        let results = extract_definitions(&CSharpParser, "anything", DefKind::all(), "");
        assert!(results.is_empty());
    }

    #[test]
    fn test_extract_with_malformed_source() {
        let results = extract_definitions(&CSharpParser, "anything", DefKind::all(), "{{{{class");
        assert!(results.len() <= 10);
    }

    #[test]
    fn test_non_const_field_skipped() {
        let results = extract_definitions(
            &CSharpParser,
            "field",
            &[DefKind::Const],
            "class Foo { public int field; }",
        );
        assert!(results.is_empty());
    }

    #[test]
    fn test_using_directive_skipped() {
        let src = "using System;\nclass App { }";
        let using_result = extract_definitions(&CSharpParser, "System", DefKind::all(), src);
        assert!(using_result.is_empty());

        let app = extract_definitions(&CSharpParser, "App", &[DefKind::Class], src);
        assert_eq!(app.len(), 1);
    }

    #[test]
    fn test_nonexistent_name() {
        let results = extract_definitions(
            &CSharpParser,
            "DoesNotExist",
            DefKind::all(),
            "class Foo { }",
        );
        assert!(results.is_empty());
    }
}
