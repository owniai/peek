use crate::model::{DefContent, DefKind};
use crate::parser::{
    LanguageParser, MatchMode, build_scope, build_scope_from_node, extract_signature_to_body,
    first_child_by_kind, first_line_of_node, flatten_bytes, line_range, node_text, node_text_ref,
    normalize_signature,
};
use tree_sitter::{Node, Parser};

pub struct PhpParser;

impl LanguageParser for PhpParser {
    fn language(&self) -> &'static str {
        "php"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &[".php", ".phtml", ".phar", ".ctp"]
    }

    fn supported_kinds(&self) -> &'static [DefKind] {
        &[
            DefKind::Function,
            DefKind::Method,
            DefKind::Class,
            DefKind::Interface,
            DefKind::Trait,
            DefKind::Enum,
            DefKind::Const,
            DefKind::Property,
            DefKind::Namespace,
            DefKind::Variant,
        ]
    }

    impl_init_parser!(tree_sitter_php::LANGUAGE_PHP, "PHP");

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

        // Walk root's direct children linearly.
        // Track current namespace for simple-syntax namespace declarations.
        // For brace-syntax, process namespace body directly.
        let mut current_ns = String::new();
        let mut cursor = root.walk();
        for child in root.children(&mut cursor) {
            match child.kind() {
                "namespace_definition" => {
                    // Extract namespace name (may be absent for global namespace {})
                    let ns_name = extract_namespace_name(child, source);

                    // Check if brace syntax: has a body field (compound_statement)
                    if let Some(body) = child.child_by_field_name("body") {
                        // Brace syntax: process body children with this namespace
                        // Emit namespace definition
                        if !ns_name.is_empty()
                            && kinds.contains(&DefKind::Namespace)
                            && mode.matches_ident(&ns_name)
                        {
                            let sig = extract_signature_to_body(child, source);
                            let start_row = child.start_position().row + 1;
                            let [start, end] = line_range(start_row, child);
                            results.push(DefContent {
                                kind: DefKind::Namespace,
                                lines: [start, end],
                                signature: sig,
                                scope: ns_name.clone(),
                            });
                        }
                        let mut body_cursor = body.walk();
                        for body_child in body.children(&mut body_cursor) {
                            collect_definitions(
                                body_child,
                                source,
                                mode,
                                kinds,
                                &mut results,
                                &ns_name,
                            );
                        }
                        // Reset current_ns after brace namespace
                        current_ns = String::new();
                    } else {
                        // Simple syntax: emit namespace definition, then update current_ns
                        if !ns_name.is_empty()
                            && kinds.contains(&DefKind::Namespace)
                            && mode.matches_ident(&ns_name)
                        {
                            let sig = normalize_signature(&first_line_of_node(child, source));
                            let start_row = child.start_position().row + 1;
                            let [start, end] = line_range(start_row, child);
                            results.push(DefContent {
                                kind: DefKind::Namespace,
                                lines: [start, end],
                                signature: sig,
                                scope: ns_name.clone(),
                            });
                        }
                        current_ns = ns_name;
                    }
                }
                "php_tag"
                | "text"
                | "text_interpolation"
                | "namespace_use_declaration"
                | "namespace_use_function_declaration"
                | "namespace_use_const_declaration"
                | "namespace_group_use_declaration" => {
                    // Skip non-definition top-level nodes
                }
                _ => {
                    collect_definitions(child, source, mode, kinds, &mut results, &current_ns);
                }
            }
        }

        Ok(results)
    }
}

/// Extract namespace name from a namespace_definition node.
/// Returns empty string if no name field (global namespace `namespace {}`).
fn extract_namespace_name(node: Node, source: &str) -> String {
    let name_node = match node.child_by_field_name("name") {
        Some(n) => n,
        None => return String::new(),
    };
    // namespace_name is a flat list of (name, "\") pairs
    // e.g. (namespace_name (name "App") "\" (name "Services"))
    extract_namespace_name_from_node(name_node, source)
}

/// Extract the full qualified name from a namespace_name node.
/// namespace_name is a flat structure: (name "X") "\" (name "Y") "\" (name "Z")
/// We collect all `name` children and join with `\`.
fn extract_namespace_name_from_node(node: Node, source: &str) -> String {
    if node.kind() == "name" {
        return node_text(node, source);
    }
    let mut parts = Vec::new();
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "name" {
            parts.push(node_text(child, source));
        }
    }
    parts.join("\\")
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
/// `scope` is the `\`-separated namespace context (e.g. "App\\Services")
/// plus `::`-separated type hierarchy (e.g. "App\\Services\\UserService::method").
fn collect_definitions(
    node: Node,
    source: &str,
    mode: &MatchMode,
    kinds: &[DefKind],
    results: &mut Vec<DefContent>,
    scope: &str,
) {
    match node.kind() {
        // --- Type definitions (class/interface/trait share same structure) ---
        "class_declaration" => {
            let own_scope = build_scope_from_node(node, source, scope, "\\");
            if kinds.contains(&DefKind::Class) {
                handle_type_definition(node, source, mode, DefKind::Class, results, &own_scope);
            }
            recurse_into_body(
                node,
                source,
                mode,
                kinds,
                results,
                &own_scope,
                "declaration_list",
            );
        }
        "interface_declaration" => {
            let own_scope = build_scope_from_node(node, source, scope, "\\");
            if kinds.contains(&DefKind::Interface) {
                handle_type_definition(node, source, mode, DefKind::Interface, results, &own_scope);
            }
            recurse_into_body(
                node,
                source,
                mode,
                kinds,
                results,
                &own_scope,
                "declaration_list",
            );
        }
        "trait_declaration" => {
            let own_scope = build_scope_from_node(node, source, scope, "\\");
            if kinds.contains(&DefKind::Trait) {
                handle_type_definition(node, source, mode, DefKind::Trait, results, &own_scope);
            }
            recurse_into_body(
                node,
                source,
                mode,
                kinds,
                results,
                &own_scope,
                "declaration_list",
            );
        }
        "enum_declaration" => {
            let own_scope = build_scope_from_node(node, source, scope, "\\");
            if kinds.contains(&DefKind::Enum) {
                handle_type_definition(node, source, mode, DefKind::Enum, results, &own_scope);
            }
            // enum body is enum_declaration_list, which contains enum_case nodes only
            // We still recurse to catch any method/const inside backed enums
            recurse_into_body(
                node,
                source,
                mode,
                kinds,
                results,
                &own_scope,
                "enum_declaration_list",
            );
        }
        // --- Method/Function definitions ---
        "method_declaration" => {
            if kinds.contains(&DefKind::Method) {
                handle_method(node, source, mode, results, scope);
            }
            // Do not recurse into method body
        }
        "function_definition" => {
            if kinds.contains(&DefKind::Function) {
                handle_function(node, source, mode, results, scope);
            }
            // Do not recurse into function body
        }
        // --- Constant definitions ---
        "const_declaration" => {
            if kinds.contains(&DefKind::Const) {
                handle_const(node, source, mode, results, scope);
            }
        }
        // --- Property definitions ---
        "property_declaration" => {
            if kinds.contains(&DefKind::Property) {
                handle_property(node, source, mode, results, scope);
            }
        }
        // --- Enum case (Variant) ---
        "enum_case" => {
            if kinds.contains(&DefKind::Variant) {
                handle_enum_case(node, source, mode, results, scope);
            }
        }
        // --- Container nodes: recurse into children ---
        "declaration_list" | "enum_declaration_list" | "compound_statement" => {
            recurse_children(node, source, mode, kinds, results, scope);
        }
        // --- Skip non-definition nodes ---
        "namespace_use_declaration"
        | "namespace_use_function_declaration"
        | "namespace_use_const_declaration"
        | "namespace_group_use_declaration"
        | "attribute_list"
        | "php_tag"
        | "text"
        | "text_interpolation"
        | "echo_statement"
        | "expression_statement"
        | "return_statement"
        | "use_declaration" => {}
        // --- Default: recurse into children ---
        _ => {
            recurse_children(node, source, mode, kinds, results, scope);
        }
    }
}

/// Recurse into a type node's body of the given kind, if present.
fn recurse_into_body(
    node: Node,
    source: &str,
    mode: &MatchMode,
    kinds: &[DefKind],
    results: &mut Vec<DefContent>,
    scope: &str,
    body_kind: &str,
) {
    if let Some(body) = first_child_by_kind(node, body_kind) {
        recurse_children(body, source, mode, kinds, results, scope);
    }
}

/// Handle a type-level definition node (class, interface, trait).
/// All three share identical AST structure: name field + body: declaration_list.
/// Signature: flatten from node start to body boundary, fallback to first line.
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

/// Extract a type definition's signature: from (attributes or node) start to the body boundary.
fn extract_type_signature(node: Node, source: &str) -> String {
    let start_byte = signature_start_byte(node);
    let mut end_byte = node.end_byte();
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "declaration_list" | "enum_declaration_list" => {
                end_byte = child.start_byte();
                break;
            }
            _ => {}
        }
    }
    flatten_bytes(start_byte, end_byte, source)
        .map(|s| normalize_signature(&s))
        .unwrap_or_else(|| first_line_of_node(node, source))
}

/// Compute the start byte for a definition's signature.
/// If the node has an `attributes` field (attribute_list), start from the
/// attribute_list's start byte; otherwise start from the node's start byte.
fn signature_start_byte(node: Node) -> usize {
    match node.child_by_field_name("attributes") {
        Some(attr) => attr.start_byte(),
        None => node.start_byte(),
    }
}

/// Handle a method_declaration node (class/trait/interface method).
/// Body is optional (abstract/interface methods have no body).
/// Signature: from (attributes or node) start to body boundary or semicolon.
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

    let own_scope = build_scope(scope, "::", name_ref);
    let signature = extract_method_signature(node, source);
    let start_row = node.start_position().row + 1;
    let [start, end] = line_range(start_row, node);

    results.push(DefContent {
        kind: DefKind::Method,
        lines: [start, end],
        signature,
        scope: own_scope,
    });
}

/// Handle a function_definition node (top-level function, always has body).
/// Signature: from (attributes or node) start to body boundary.
fn handle_function(
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

    let own_scope = build_scope(scope, "\\", name_ref);
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

/// Handle a const_declaration node.
/// May contain multiple const_element children (e.g., `const A = 1, B = 2;`).
/// Each const_element's name node has no field association; we iterate
/// named children to find the first one with kind "name".
fn handle_const(
    node: Node,
    source: &str,
    mode: &MatchMode,
    results: &mut Vec<DefContent>,
    scope: &str,
) {
    // Determine separator: inside a type body (class/interface/trait/enum) use "::", otherwise use "\" (namespace)
    let sep = match node.parent() {
        Some(p) if p.kind() == "declaration_list" || p.kind() == "enum_declaration_list" => "::",
        _ => "\\",
    };

    let sig = extract_const_signature(node, source);
    let start_row = node.start_position().row + 1;
    let [start, end] = line_range(start_row, node);

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() != "const_element" {
            continue;
        }
        let name_ref = match extract_const_name_ref(child, source) {
            Some(n) => n,
            None => continue,
        };
        if !mode.matches_ident(name_ref) {
            continue;
        }

        let own_scope = build_scope(scope, sep, name_ref);
        results.push(DefContent {
            kind: DefKind::Const,
            lines: [start, end],
            signature: sig.clone(),
            scope: own_scope,
        });
    }
}

/// Extract a method's signature.
/// - Has body: truncate to body start byte
/// - No body (abstract/interface): truncate to first `;` child
/// - Includes leading attributes if present
fn extract_method_signature(node: Node, source: &str) -> String {
    let start_byte = signature_start_byte(node);
    let end_byte = if let Some(body) = node.child_by_field_name("body") {
        body.start_byte()
    } else {
        // Abstract/interface method: find semicolon to truncate before it
        let mut end = node.end_byte();
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == ";" {
                end = child.start_byte();
                break;
            }
        }
        end
    };
    flatten_bytes(start_byte, end_byte, source)
        .map(|s| normalize_signature(&s))
        .unwrap_or_else(|| first_line_of_node(node, source))
}

/// Extract the constant name from a const_element node.
/// The name node is a named child without field association.
/// We iterate children and find the first one with kind "name".
fn extract_const_name_ref<'a>(node: Node, source: &'a str) -> Option<&'a str> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "name" {
            return Some(node_text_ref(child, source));
        }
    }
    None
}

/// Extract a const_declaration's signature: from node start to `;` boundary.
fn extract_const_signature(node: Node, source: &str) -> String {
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

/// Handle a property_declaration node: extract typed properties.
/// PHP property_declaration > property_element > name > variable_name > name
fn handle_property(
    node: Node,
    source: &str,
    mode: &MatchMode,
    results: &mut Vec<DefContent>,
    scope: &str,
) {
    let sig = first_line_of_node(node, source);
    let signature = normalize_signature(&sig);
    let start_row = node.start_position().row + 1;
    let [start, end] = line_range(start_row, node);

    // Find property_element children and extract variable names
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() != "property_element" {
            continue;
        }
        let name_node = match child.child_by_field_name("name") {
            Some(n) => n,
            None => continue,
        };

        // Resolve the property name (without $ prefix)
        let name_ref: &str = if name_node.kind() == "variable_name" {
            match name_node.child_by_field_name("name") {
                Some(n) => node_text_ref(n, source),
                None => {
                    // Fallback: strip $ from variable_name text
                    let raw = node_text_ref(name_node, source);
                    raw.strip_prefix('$').unwrap_or(raw)
                }
            }
        } else {
            node_text_ref(name_node, source)
        };

        if !mode.matches_ident(name_ref) {
            continue;
        }

        let own_scope = build_scope(scope, "::", name_ref);
        results.push(DefContent {
            kind: DefKind::Property,
            lines: [start, end],
            signature: signature.clone(),
            scope: own_scope,
        });
    }
}

/// Handle an enum_case node (PHP enum variant).
/// enum_case has a required `name` field (type: "name") and optional `value` field.
/// Signature: from (attributes or node) start to `;` boundary.
fn handle_enum_case(
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

    let own_scope = build_scope(scope, "::", name_ref);
    let signature = extract_enum_case_signature(node, source);
    let start_row = node.start_position().row + 1;
    let [start, end] = line_range(start_row, node);

    results.push(DefContent {
        kind: DefKind::Variant,
        lines: [start, end],
        signature,
        scope: own_scope,
    });
}

/// Extract an enum_case's signature: from (attributes or node) start to `;` boundary.
fn extract_enum_case_signature(node: Node, source: &str) -> String {
    let start_byte = signature_start_byte(node);
    let mut end_byte = node.end_byte();
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == ";" {
            end_byte = child.start_byte();
            break;
        }
    }
    flatten_bytes(start_byte, end_byte, source)
        .map(|s| normalize_signature(&s))
        .unwrap_or_else(|| first_line_of_node(node, source))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::extract_definitions;

    // === Edge case tests ===

    #[test]
    fn test_empty_php_file() {
        let results = extract_definitions(&PhpParser, "anything", DefKind::all(), "<?php");
        assert!(results.is_empty());
    }

    #[test]
    fn test_malformed_source() {
        let results =
            extract_definitions(&PhpParser, "anything", DefKind::all(), "<?php {{{{class");
        assert!(results.len() <= 10);
    }

    #[test]
    fn test_enum_case_extracted_as_variant() {
        let results = extract_definitions(
            &PhpParser,
            "Active",
            DefKind::all(),
            "<?php enum Status { case Active; case Inactive; }",
        );
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].kind, DefKind::Variant);
        assert_eq!(results[0].scope, "Status::Active");
        assert_eq!(results[0].signature, "case Active");
    }

    #[test]
    fn test_enum_case_not_extracted_as_const() {
        let src = "<?php enum Status { case Active; case Inactive; }";
        let results = extract_definitions(&PhpParser, "Active", &[DefKind::Const], src);
        assert!(results.is_empty());
    }

    #[test]
    fn test_function_kind_filter() {
        let src = "<?php class Foo { public function bar() {} }";
        let results = extract_definitions(&PhpParser, "bar", &[DefKind::Class], src);
        assert!(results.is_empty());
    }

    #[test]
    fn test_const_name_mismatch() {
        let src = "<?php const MAX = 100;";
        let results = extract_definitions(&PhpParser, "MIN", &[DefKind::Const], src);
        assert!(results.is_empty());
    }

    #[test]
    fn test_multi_line_class_signature() {
        let src = "<?php\nabstract\nclass Shape\n{ }";
        let results = extract_definitions(&PhpParser, "Shape", &[DefKind::Class], src);
        assert_eq!(results.len(), 1);
        assert!(!results[0].signature.contains('\n'));
        assert_eq!(results[0].signature, "abstract class Shape");
    }

    #[test]
    fn test_multi_line_function_signature() {
        let src = "<?php
function greet(
    string $name,
    int $age
): string { return ''; }";
        let results = extract_definitions(&PhpParser, "greet", &[DefKind::Function], src);
        assert_eq!(results.len(), 1);
        assert!(!results[0].signature.contains('\n'));
        assert!(results[0].signature.contains("function greet"));
    }

    // === MatchMode edge case tests ===

    #[test]
    fn test_exact_match_mode() {
        let src = "<?php class UserService { } class BaseService { }";
        let mode = MatchMode::from_user_input("UserService", false, false).unwrap();
        let p = PhpParser;
        let mut parser = p.init_parser();
        let results = p
            .extract_with(&mode, &[DefKind::Class], src, &mut parser)
            .unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].signature, "class UserService");
    }

    #[test]
    fn test_fuzzy_match_mode() {
        let src = "<?php class UserService { } class UserFactory { } class BaseService { }";
        let mode = MatchMode::from_user_input("User.*", false, false).unwrap();
        let p = PhpParser;
        let mut parser = p.init_parser();
        let results = p
            .extract_with(&mode, &[DefKind::Class], src, &mut parser)
            .unwrap();
        assert_eq!(results.len(), 2);
    }

    // === HTML mixed content ===

    #[test]
    fn test_php_html_mixed() {
        let src = "<html><body><?php class UserService { } ?></body></html>";
        let results = extract_definitions(&PhpParser, "UserService", &[DefKind::Class], src);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].kind, DefKind::Class);
    }
}
