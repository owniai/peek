use crate::model::{DefContent, DefKind};
use crate::parser::{
    LanguageParser, MatchMode, build_scope, build_scope_from_node, extract_signature_to_body,
    first_child_by_kind, first_line_of_node, flatten_bytes, line_range, node_text, node_text_ref,
    normalize_signature,
};
use tree_sitter::{Node, Parser};

pub struct CSharpParser;

impl LanguageParser for CSharpParser {
    fn language(&self) -> &'static str {
        "csharp"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &[".cs", ".csx", ".cake", ".linq"]
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
            DefKind::Constructor,
            DefKind::Getter,
            DefKind::Setter,
            DefKind::Operator,
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
            if kinds.contains(&DefKind::Record) {
                handle_type_definition(node, source, mode, DefKind::Record, results, &own_scope);
            }
            // record may have a body (declaration_list) or end with `;`
            recurse_into_type_body(node, source, mode, kinds, results, &own_scope);
        }
        // --- Method ---
        "method_declaration" => {
            if kinds.contains(&DefKind::Method) {
                handle_method(node, source, mode, DefKind::Method, results, scope);
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
        // --- Subscript (indexer) ---
        "indexer_declaration" => {
            if kinds.contains(&DefKind::Subscript) {
                handle_indexer(node, source, mode, results, scope);
            }
        }
        // --- Operator ---
        "operator_declaration" => {
            if kinds.contains(&DefKind::Operator) {
                handle_operator(node, source, mode, results, scope);
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

    let signature = extract_signature_to_semicolon(node, source);
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
            signature: signature.clone(),
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

/// Handle an operator_declaration node: extract as Operator kind.
/// C# operator overloading uses `public static ReturnType operator +(T a, T b)`.
/// The name format is "operator+", "operator==", etc.
fn handle_operator(
    node: Node,
    source: &str,
    mode: &MatchMode,
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
        kind: DefKind::Operator,
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

    // === Sub-kind classification tests ===

    #[test]
    fn test_method_is_method_kind() {
        let src = "class Foo { public void Bar() { } }";
        let defs = extract_definitions(&CSharpParser, "Bar", &[DefKind::Method], src);
        assert_eq!(defs.len(), 1);
        assert_eq!(defs[0].kind, DefKind::Method);
        assert_eq!(defs[0].scope, "Foo.Bar");
    }

    #[test]
    fn test_constructor_is_constructor_kind() {
        let src = "class Foo { public Foo() { } }";
        let defs = extract_definitions(&CSharpParser, "Foo", &[DefKind::Constructor], src);
        assert_eq!(defs.len(), 1);
        assert_eq!(defs[0].kind, DefKind::Constructor);
        assert_eq!(defs[0].scope, "Foo.Foo");
    }

    #[test]
    fn test_function_kind_excludes_method() {
        let src = "class Foo { public void Bar() { } }";
        let defs = extract_definitions(&CSharpParser, "Bar", &[DefKind::Function], src);
        assert!(defs.is_empty(), "method should not match Function kind");
    }

    #[test]
    fn test_property_getter_is_getter_kind() {
        let src = "class Foo { public string Name { get; } }";
        let defs = extract_definitions(&CSharpParser, "Name", &[DefKind::Getter], src);
        assert_eq!(defs.len(), 1);
        assert_eq!(defs[0].kind, DefKind::Getter);
    }

    #[test]
    fn test_property_setter_is_setter_kind() {
        let src = "class Foo { public string Name { set; } }";
        let defs = extract_definitions(&CSharpParser, "Name", &[DefKind::Setter], src);
        assert_eq!(defs.len(), 1);
        assert_eq!(defs[0].kind, DefKind::Setter);
    }

    #[test]
    fn test_property_getter_setter_both_extracted() {
        let src = "class Foo { public string Name { get; set; } }";
        let defs = extract_definitions(
            &CSharpParser,
            "Name",
            &[DefKind::Getter, DefKind::Setter],
            src,
        );
        assert_eq!(defs.len(), 2);
        assert!(defs.iter().any(|d| d.kind == DefKind::Getter));
        assert!(defs.iter().any(|d| d.kind == DefKind::Setter));
    }

    #[test]
    fn test_property_with_bodies() {
        let src =
            "class Foo { public string Name { get { return _name; } set { _name = value; } } }";
        let defs = extract_definitions(
            &CSharpParser,
            "Name",
            &[DefKind::Getter, DefKind::Setter],
            src,
        );
        assert_eq!(defs.len(), 2);
    }

    #[test]
    fn test_callable_includes_all_sub_kinds() {
        let src =
            "class Foo { public Foo() { } public void Bar() { } public string Name { get; set; } }";
        let all = &[
            DefKind::Method,
            DefKind::Constructor,
            DefKind::Getter,
            DefKind::Setter,
        ];
        let ctor = extract_definitions(&CSharpParser, "Foo", all, src);
        assert_eq!(ctor.len(), 1);
        assert_eq!(ctor[0].kind, DefKind::Constructor);

        let bar = extract_definitions(&CSharpParser, "Bar", all, src);
        assert_eq!(bar.len(), 1);
        assert_eq!(bar[0].kind, DefKind::Method);

        let name = extract_definitions(&CSharpParser, "Name", all, src);
        assert_eq!(name.len(), 2);
    }

    // === Variant (enum member) tests ===

    #[test]
    fn test_enum_member_is_variant_kind() {
        let src = "enum Color { Red, Green, Blue }";
        let defs = extract_definitions(&CSharpParser, "Red", &[DefKind::Variant], src);
        assert_eq!(defs.len(), 1);
        assert_eq!(defs[0].kind, DefKind::Variant);
        assert_eq!(defs[0].scope, "Color.Red");
    }

    #[test]
    fn test_enum_members_all_extracted() {
        let src = "enum Direction { North, South, East, West }";
        let defs = extract_definitions(&CSharpParser, "Direction", &[DefKind::Variant], src);
        // "Direction" doesn't match any variant name, so should be empty
        // (we're searching by name, and variants are named North/South/etc.)
        assert!(
            defs.is_empty(),
            "Direction matches the enum itself, not variants"
        );

        let north = extract_definitions(&CSharpParser, "North", &[DefKind::Variant], src);
        assert_eq!(north.len(), 1);
        assert_eq!(north[0].scope, "Direction.North");

        let south = extract_definitions(&CSharpParser, "South", &[DefKind::Variant], src);
        assert_eq!(south.len(), 1);
        assert_eq!(south[0].scope, "Direction.South");
    }

    #[test]
    fn test_enum_member_with_value() {
        let src = "enum HttpStatus { OK = 200, NotFound = 404 }";
        let defs = extract_definitions(&CSharpParser, "OK", &[DefKind::Variant], src);
        assert_eq!(defs.len(), 1);
        assert_eq!(defs[0].kind, DefKind::Variant);
        assert_eq!(defs[0].scope, "HttpStatus.OK");
    }

    #[test]
    fn test_variant_kind_filtered_out() {
        let src = "enum Color { Red }";
        let defs = extract_definitions(&CSharpParser, "Red", &[DefKind::Class], src);
        assert!(defs.is_empty(), "variant should not match Class kind");
    }

    // === Subscript (indexer) tests ===

    #[test]
    fn test_indexer_is_subscript_kind() {
        let src = "class MyList { public int this[int index] { get { return _items[index]; } } }";
        let defs = extract_definitions(&CSharpParser, "this", &[DefKind::Subscript], src);
        assert_eq!(defs.len(), 1);
        assert_eq!(defs[0].kind, DefKind::Subscript);
        assert_eq!(defs[0].scope, "MyList.this");
    }

    #[test]
    fn test_indexer_with_setter() {
        let src = "class MyList { public string this[int i] { get { return \"\"; } set { } } }";
        let defs = extract_definitions(&CSharpParser, "this", &[DefKind::Subscript], src);
        assert_eq!(defs.len(), 1);
        assert_eq!(defs[0].kind, DefKind::Subscript);
    }

    #[test]
    fn test_subscript_kind_filtered_out() {
        let src = "class MyList { public int this[int index] { get { return 0; } } }";
        let defs = extract_definitions(&CSharpParser, "this", &[DefKind::Method], src);
        assert!(defs.is_empty(), "indexer should not match Method kind");
    }

    // === Operator tests ===

    #[test]
    fn test_binary_operator_is_operator_kind() {
        let src =
            "class Vector { public static Vector operator +(Vector a, Vector b) { return a; } }";
        let defs = extract_definitions(&CSharpParser, "operator", &[DefKind::Operator], src);
        assert_eq!(defs.len(), 1);
        assert_eq!(defs[0].kind, DefKind::Operator);
        assert_eq!(defs[0].scope, "Vector.operator+");
    }

    #[test]
    fn test_comparison_operator_is_operator_kind() {
        let src =
            "class Money { public static bool operator ==(Money a, Money b) { return true; } }";
        let defs = extract_definitions(&CSharpParser, "operator", &[DefKind::Operator], src);
        assert_eq!(defs.len(), 1);
        assert_eq!(defs[0].kind, DefKind::Operator);
        assert_eq!(defs[0].scope, "Money.operator==");
    }

    #[test]
    fn test_operator_kind_filtered_out() {
        let src =
            "class Vector { public static Vector operator +(Vector a, Vector b) { return a; } }";
        let defs = extract_definitions(&CSharpParser, "operator", &[DefKind::Method], src);
        assert!(defs.is_empty(), "operator should not match Method kind");
    }

    // === Destructor tests ===

    #[test]
    fn test_destructor_is_destructor_kind() {
        let src = "class Foo { ~Foo() { } }";
        let defs = extract_definitions(&CSharpParser, "Foo", &[DefKind::Destructor], src);
        assert_eq!(defs.len(), 1);
        assert_eq!(defs[0].kind, DefKind::Destructor);
        assert_eq!(defs[0].scope, "Foo.Foo");
    }

    #[test]
    fn test_destructor_kind_filtered_out() {
        let src = "class Foo { ~Foo() { } }";
        let defs = extract_definitions(&CSharpParser, "Foo", &[DefKind::Method], src);
        // Constructor also matches "Foo" but not destructor
        // Only if constructor exists; here there's no constructor, so nothing matches Method
        assert!(defs.is_empty(), "destructor should not match Method kind");
    }

    #[test]
    fn test_destructor_in_namespace() {
        let src = "namespace MyApp { class Resource { ~Resource() { Cleanup(); } } }";
        let defs = extract_definitions(&CSharpParser, "Resource", &[DefKind::Destructor], src);
        assert_eq!(defs.len(), 1);
        assert_eq!(defs[0].kind, DefKind::Destructor);
        assert_eq!(defs[0].scope, "MyApp.Resource.Resource");
    }
}
