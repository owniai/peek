use crate::model::{DefContent, DefKind};
use crate::parser::{
    LanguageParser, MatchMode, build_scope, build_scope_from_node, extract_signature_to_body,
    first_child_by_kind, first_line_of_node, flatten_bytes, line_range, node_text, node_text_ref,
    normalize_signature,
};
use tree_sitter::{Node, Parser};

pub(crate) const LANGUAGE: &str = "csharp";
pub(crate) const EXTENSIONS: &[&str] = &["cs", "csx", "cake", "linq"];
pub(crate) const ALIASES: &[&str] = &["cs", "c#", "c-sharp"];

pub struct CSharpParser;

impl LanguageParser for CSharpParser {
    fn language(&self) -> &'static str {
        LANGUAGE
    }

    fn extensions(&self) -> &'static [&'static str] {
        EXTENSIONS
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
            DefKind::Method,
            DefKind::MethodDeclaration,
            DefKind::Constructor,
            DefKind::Getter,
            DefKind::Setter,
            DefKind::Operator,
            DefKind::OperatorDeclaration,
            DefKind::Destructor,
            DefKind::Subscript,
            DefKind::Const,
            DefKind::Field,
            DefKind::Property,
            DefKind::Variant,
            DefKind::Namespace,
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
                    if kinds.contains(&DefKind::Namespace) && mode.matches_ident(&ns) {
                        let sig = normalize_signature(&first_line_of_node(child, source));
                        let start_row = child.start_position().row + 1;
                        let [start, end] = line_range(start_row, child);
                        results.push(DefContent {
                            kind: DefKind::Namespace,
                            lines: [start, end],
                            signature: sig,
                            scope: ns.clone(),
                        });
                    }
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
            if kinds.contains(&DefKind::Namespace) && mode.matches_ident(&new_scope) {
                let sig = extract_signature_to_body(node, source);
                let start_row = node.start_position().row + 1;
                let [start, end] = line_range(start_row, node);
                results.push(DefContent {
                    kind: DefKind::Namespace,
                    lines: [start, end],
                    signature: sig,
                    scope: new_scope.clone(),
                });
            }
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
            let def_kind = if has_record_struct_modifier(node) {
                DefKind::Struct
            } else {
                DefKind::Record
            };
            if kinds.contains(&def_kind) {
                handle_type_definition(node, source, mode, def_kind, results, &own_scope);
            }
            // record may have a body (declaration_list) or end with `;`
            recurse_into_type_body(node, source, mode, kinds, results, &own_scope);
        }
        // --- Method ---
        "method_declaration" => {
            let has_body = node.child_by_field_name("body").is_some();
            let def_kind = if has_body {
                DefKind::Method
            } else {
                DefKind::MethodDeclaration
            };
            if kinds.contains(&def_kind) {
                handle_method(node, source, mode, def_kind, results, scope);
            }
            // Do not recurse into method body
        }
        // --- Constructor ---
        "constructor_declaration" => {
            if kinds.contains(&DefKind::Constructor) {
                handle_method(node, source, mode, DefKind::Constructor, results, scope);
            }
            // Do not recurse into constructor body
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
        "event_declaration" => {
            if kinds.contains(&DefKind::Event) {
                handle_event_declaration(node, source, mode, results, scope);
            }
        }
        // --- Const field / Field ---
        "field_declaration" => {
            if is_const_field(node) {
                if kinds.contains(&DefKind::Const) {
                    push_const_declarators(node, source, mode, results, scope);
                }
            } else if kinds.contains(&DefKind::Field) {
                push_field_declarators(node, source, mode, results, scope);
            }
        }
        // --- Enum member (Variant) ---
        "enum_member_declaration" => {
            if kinds.contains(&DefKind::Variant) {
                handle_enum_member(node, source, mode, results, scope);
            }
        }
        // --- Subscript (indexer) / Indexer accessors (getter/setter) ---
        "indexer_declaration" => {
            if kinds.contains(&DefKind::Subscript) {
                handle_indexer(node, source, mode, results, scope);
            }
            handle_indexer_accessors(node, source, mode, kinds, results, scope);
        }
        // --- Operator ---
        "operator_declaration" => {
            let has_body = node.child_by_field_name("body").is_some();
            let def_kind = if has_body {
                DefKind::Operator
            } else {
                DefKind::OperatorDeclaration
            };
            if kinds.contains(&def_kind) {
                handle_operator_with_kind(node, source, mode, def_kind, results, scope);
            }
        }
        // --- Conversion operator (implicit/explicit) ---
        "conversion_operator_declaration" => {
            let has_body = node.child_by_field_name("body").is_some();
            let def_kind = if has_body {
                DefKind::Operator
            } else {
                DefKind::OperatorDeclaration
            };
            if kinds.contains(&def_kind) {
                handle_conversion_operator_with_kind(node, source, mode, def_kind, results, scope);
            }
        }
        // --- Destructor ---
        "destructor_declaration" => {
            if kinds.contains(&DefKind::Destructor) {
                handle_destructor(node, source, mode, results, scope);
            }
        }
        // --- Property / Property accessors (getter/setter) ---
        "property_declaration" => {
            if kinds.contains(&DefKind::Property) {
                handle_property(node, source, mode, results, scope);
            }
            handle_property_accessors(node, source, mode, kinds, results, scope);
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

/// Check if a record_declaration has a `struct` modifier (i.e. `record struct`).
fn has_record_struct_modifier(node: Node) -> bool {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "struct" {
            return true;
        }
    }
    false
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

/// Handle a method_declaration or constructor_declaration node.
/// Signature: from node start to body boundary, fallback to first line.
fn handle_method(
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

    let own_scope = build_scope_from_node(node, source, scope, ".");
    let signature = extract_method_signature(node, source);
    let start_row = node.start_position().row + 1;
    let [start, end] = line_range(start_row, node);

    results.push(DefContent {
        kind: def_kind,
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

/// Handle property_declaration: extract the property as Property kind.
fn handle_property(
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

    let own_scope = build_scope(scope, ".", name_ref);
    let signature = extract_signature_to_semicolon(node, source);
    let start_row = node.start_position().row + 1;
    let [start, end] = line_range(start_row, node);

    results.push(DefContent {
        kind: DefKind::Property,
        lines: [start, end],
        signature,
        scope: own_scope,
    });
}

/// Handle property_declaration: extract getter/setter from accessor_list.
/// Accessor name field is "get" or "set". Property name comes from property_declaration's name field.
fn handle_property_accessors(
    node: Node,
    source: &str,
    mode: &MatchMode,
    kinds: &[DefKind],
    results: &mut Vec<DefContent>,
    scope: &str,
) {
    let prop_name_node = match node.child_by_field_name("name") {
        Some(n) => n,
        None => return,
    };
    let prop_name_ref = node_text_ref(prop_name_node, source);

    // Find accessor_list via the "accessors" field
    let accessor_list = match node.child_by_field_name("accessors") {
        Some(al) => al,
        None => return,
    };

    let own_scope = build_scope(scope, ".", prop_name_ref);

    // Match on property name before iterating accessors
    if !mode.matches_ident(prop_name_ref) {
        return;
    }

    let mut cursor = accessor_list.walk();
    for child in accessor_list.children(&mut cursor) {
        if child.kind() != "accessor_declaration" {
            continue;
        }

        let accessor_name = match child.child_by_field_name("name") {
            Some(n) => node_text_ref(n, source).to_string(),
            None => continue,
        };

        let def_kind = match accessor_name.as_str() {
            "get" => DefKind::Getter,
            "set" => DefKind::Setter,
            _ => continue,
        };

        if !kinds.contains(&def_kind) {
            continue;
        }

        let start_row = child.start_position().row + 1;
        let [start, end] = line_range(start_row, child);

        results.push(DefContent {
            kind: def_kind,
            lines: [start, end],
            signature: first_line_of_node(child, source),
            scope: own_scope.clone(),
        });
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

/// Handle an event_declaration node (custom event with add/remove accessors).
/// Name is directly in the "name" field (unlike event_field_declaration which uses variable_declaration).
/// Signature: from node start to accessor_list or `;`.
fn handle_event_declaration(
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

    let own_scope = build_scope(scope, ".", name_ref);
    let signature = if let Some(accessors) = node.child_by_field_name("accessors") {
        flatten_bytes(node.start_byte(), accessors.start_byte(), source)
            .map(|s| normalize_signature(&s))
            .unwrap_or_else(|| first_line_of_node(node, source))
    } else {
        extract_signature_to_semicolon(node, source)
    };
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

/// Iterate over variable_declarator children of a field_declaration,
/// pushing a Field Definition for each one whose name matches `mode`.
fn push_field_declarators(
    node: Node,
    source: &str,
    mode: &MatchMode,
    results: &mut Vec<DefContent>,
    scope: &str,
) {
    let sig = extract_signature_to_semicolon(node, source);
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
                kind: DefKind::Field,
                lines: [start, end],
                signature: sig.clone(),
                scope: own_scope,
            });
        }
    }
}

/// Handle an enum_member_declaration node: extract as Variant kind.
/// C# enum members are simple named items inside enum_member_declaration_list.
fn handle_enum_member(
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

    let own_scope = build_scope(scope, ".", name_ref);
    let signature = extract_signature_to_semicolon(node, source);
    let start_row = node.start_position().row + 1;
    let [start, end] = line_range(start_row, node);

    results.push(DefContent {
        kind: DefKind::Variant,
        lines: [start, end],
        signature,
        scope: own_scope,
    });
}

/// Handle an indexer_declaration node: extract as Subscript kind.
/// C# indexers use `this[T]` syntax. The name is always "this".
fn handle_indexer(
    node: Node,
    source: &str,
    mode: &MatchMode,
    results: &mut Vec<DefContent>,
    scope: &str,
) {
    let name = "this";
    if !mode.matches_ident(name) {
        return;
    }

    let own_scope = build_scope(scope, ".", name);
    let signature = extract_signature_to_semicolon(node, source);
    let start_row = node.start_position().row + 1;
    let [start, end] = line_range(start_row, node);

    results.push(DefContent {
        kind: DefKind::Subscript,
        lines: [start, end],
        signature,
        scope: own_scope,
    });
}

fn handle_indexer_accessors(
    node: Node,
    source: &str,
    mode: &MatchMode,
    kinds: &[DefKind],
    results: &mut Vec<DefContent>,
    scope: &str,
) {
    let name = "this";
    if !mode.matches_ident(name) {
        return;
    }

    let accessor_list = match node.child_by_field_name("accessors") {
        Some(al) => al,
        None => return,
    };

    let own_scope = build_scope(scope, ".", name);
    let mut cursor = accessor_list.walk();
    for child in accessor_list.children(&mut cursor) {
        if child.kind() != "accessor_declaration" {
            continue;
        }

        let accessor_name = match child.child_by_field_name("name") {
            Some(n) => node_text_ref(n, source).to_string(),
            None => continue,
        };

        let def_kind = match accessor_name.as_str() {
            "get" => DefKind::Getter,
            "set" => DefKind::Setter,
            _ => continue,
        };

        if !kinds.contains(&def_kind) {
            continue;
        }

        let start_row = child.start_position().row + 1;
        let [start, end] = line_range(start_row, child);

        results.push(DefContent {
            kind: def_kind,
            lines: [start, end],
            signature: first_line_of_node(child, source),
            scope: own_scope.clone(),
        });
    }
}

/// Handle an operator_declaration node: extract as Operator or OperatorDeclaration kind.
/// C# operator overloading uses `public static ReturnType operator +(T a, T b)`.
/// The name format is "operator+", "operator==", etc.
fn handle_operator_with_kind(
    node: Node,
    source: &str,
    mode: &MatchMode,
    def_kind: DefKind,
    results: &mut Vec<DefContent>,
    scope: &str,
) {
    let operator_name = extract_csharp_operator_name(node, source);
    if !mode.matches_ident(&operator_name) {
        return;
    }

    let own_scope = build_scope(scope, ".", &operator_name);
    let signature = extract_method_signature(node, source);
    let start_row = node.start_position().row + 1;
    let [start, end] = line_range(start_row, node);

    results.push(DefContent {
        kind: def_kind,
        lines: [start, end],
        signature,
        scope: own_scope,
    });
}

/// Extract operator name from operator_declaration node.
/// The operator_declaration has a "operator" field child for the operator token,
/// followed by the actual operator symbol. Returns "operator+" etc.
fn extract_csharp_operator_name(node: Node, source: &str) -> String {
    let mut cursor = node.walk();
    let mut prev_was_operator = false;
    for child in node.children(&mut cursor) {
        if prev_was_operator {
            let symbol = node_text_ref(child, source);
            return format!("operator{}", symbol);
        }
        if child.kind() == "operator" {
            prev_was_operator = true;
        }
    }
    "operator".to_string()
}

/// Handle a conversion_operator_declaration node (implicit/explicit operator).
/// Name format: "implicit operator int" / "explicit operator MyType".
fn handle_conversion_operator_with_kind(
    node: Node,
    source: &str,
    mode: &MatchMode,
    def_kind: DefKind,
    results: &mut Vec<DefContent>,
    scope: &str,
) {
    let name = extract_conversion_operator_name(node, source);
    if !mode.matches_ident(&name) {
        return;
    }

    let own_scope = build_scope(scope, ".", &name);
    let signature = extract_method_signature(node, source);
    let start_row = node.start_position().row + 1;
    let [start, end] = line_range(start_row, node);

    results.push(DefContent {
        kind: def_kind,
        lines: [start, end],
        signature,
        scope: own_scope,
    });
}

/// Extract conversion operator name: "implicit operator {type}" or "explicit operator {type}".
/// tree-sitter-c-sharp guarantees "implicit" or "explicit" is present as an anonymous child.
fn extract_conversion_operator_name(node: Node, source: &str) -> String {
    let mut direction = "";
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        let kind = child.kind();
        if kind == "implicit" || kind == "explicit" {
            direction = kind;
            break;
        }
    }

    if let Some(type_node) = node.child_by_field_name("type") {
        let type_text = node_text(type_node, source);
        format!("{} operator {}", direction, type_text)
    } else {
        format!("{} operator", direction)
    }
}

/// Handle a destructor_declaration node: extract as Destructor kind.
/// C# destructors are `~ClassName()` syntax.
fn handle_destructor(
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

    let own_scope = build_scope(scope, ".", name_ref);
    let signature = extract_method_signature(node, source);
    let start_row = node.start_position().row + 1;
    let [start, end] = line_range(start_row, node);

    results.push(DefContent {
        kind: DefKind::Destructor,
        lines: [start, end],
        signature,
        scope: own_scope,
    });
}
