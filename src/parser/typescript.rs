use crate::model::{DefContent, DefKind};
use crate::parser::{
    LanguageParser, MatchMode, build_scope, build_scope_from_node, classify_method_definition,
    export_aware_sig_node, first_child_by_kind, first_line_of_node, flatten_bytes, handle_pair,
    handle_var_decl, line_range, node_text_ref,
};
use tree_sitter::{Node, Parser};

pub(crate) const LANGUAGE: &str = "typescript";
pub(crate) const EXTENSIONS: &[&str] = &["ts", "tsx", "mts", "cts"];
pub(crate) const ALIASES: &[&str] = &["ts"];

pub struct TsParser;

impl LanguageParser for TsParser {
    fn language(&self) -> &'static str {
        LANGUAGE
    }

    fn extensions(&self) -> &'static [&'static str] {
        EXTENSIONS
    }

    fn supported_kinds(&self) -> &'static [DefKind] {
        &[
            DefKind::Function,
            DefKind::FunctionDeclaration,
            DefKind::Method,
            DefKind::MethodDeclaration,
            DefKind::Constructor,
            DefKind::ConstructorDeclaration,
            DefKind::Getter,
            DefKind::GetterDeclaration,
            DefKind::Setter,
            DefKind::SetterDeclaration,
            DefKind::Class,
            DefKind::ClassDeclaration,
            DefKind::Const,
            DefKind::ConstDeclaration,
            DefKind::Var,
            DefKind::VarDeclaration,
            DefKind::Interface,
            DefKind::Alias,
            DefKind::Enum,
            DefKind::EnumDeclaration,
            DefKind::Field,
            DefKind::Property,
            DefKind::PropertyDeclaration,
            DefKind::Namespace,
            DefKind::ModuleDeclaration,
            DefKind::Variant,
            DefKind::Subscript,
            DefKind::SubscriptDeclaration,
        ]
    }

    impl_init_parser!(tree_sitter_typescript::LANGUAGE_TSX, "TypeScript");

    impl_extract_with!(collect_definitions, scope: "");
}

/// Handle an `ambient_declaration` node wrapping a class or enum declaration.
/// The signature starts from the `ambient_declaration` node (which includes `declare`)
/// but the kind and body are determined by the inner child node.
fn handle_ambient_declaration<'a>(
    node: Node<'a>,
    source: &str,
    mode: &MatchMode,
    kinds: &[DefKind],
    results: &mut Vec<DefContent>,
    scope: &str,
) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        let (target_kind, body_kind) = match child.kind() {
            "class_declaration" | "abstract_class_declaration" => {
                (DefKind::ClassDeclaration, "class_body")
            }
            "enum_declaration" => (DefKind::EnumDeclaration, "enum_body"),
            "lexical_declaration" | "variable_declaration" => {
                handle_ambient_lexical(node, child, source, mode, kinds, results, scope);
                continue;
            }
            _ => {
                collect_definitions(child, source, mode, kinds, results, scope);
                continue;
            }
        };

        let name_node = match child.child_by_field_name("name") {
            Some(n) => n,
            None => continue,
        };
        let name_ref = node_text_ref(name_node, source);
        if !mode.matches_ident(name_ref) {
            continue;
        }
        let name = name_ref.to_string();
        let new_scope = build_scope(scope, ".", &name);

        if kinds.contains(&target_kind) {
            let body = first_child_by_kind(child, body_kind);
            let end_byte = body
                .map(|b| b.start_byte())
                .unwrap_or_else(|| child.end_byte());
            let signature = flatten_bytes(node.start_byte(), end_byte, source)
                .unwrap_or_else(|| first_line_of_node(node, source));
            let start_row = node.start_position().row + 1;
            let [start, end] = line_range(start_row, child);

            results.push(DefContent {
                kind: target_kind,
                lines: [start, end],
                signature,
                scope: new_scope.clone(),
            });
        }

        // Recurse into body for nested definitions
        if let Some(body) = first_child_by_kind(child, body_kind) {
            let mut bc = body.walk();
            for gc in body.children(&mut bc) {
                collect_definitions(gc, source, mode, kinds, results, &new_scope);
            }
        }
    }
}

fn handle_ambient_lexical<'a>(
    ambient: Node<'a>,
    lexical: Node<'a>,
    source: &str,
    mode: &MatchMode,
    kinds: &[DefKind],
    results: &mut Vec<DefContent>,
    scope: &str,
) {
    let is_const = first_child_by_kind(lexical, "const").is_some();
    let decl_kind = if is_const {
        DefKind::ConstDeclaration
    } else {
        DefKind::VarDeclaration
    };

    let mut cursor = lexical.walk();
    for child in lexical.children(&mut cursor) {
        if child.kind() != "variable_declarator" {
            continue;
        }
        let name_node = match child.child_by_field_name("name") {
            Some(n) => n,
            None => continue,
        };
        let name_ref = node_text_ref(name_node, source);
        if !mode.matches_ident(name_ref) {
            continue;
        }
        let own_scope = build_scope(scope, ".", name_ref);

        if kinds.contains(&decl_kind) {
            let signature = flatten_bytes(ambient.start_byte(), child.end_byte(), source)
                .unwrap_or_else(|| first_line_of_node(ambient, source));
            let start_row = ambient.start_position().row + 1;
            let [start, end] = line_range(start_row, lexical);

            results.push(DefContent {
                kind: decl_kind,
                lines: [start, end],
                signature,
                scope: own_scope,
            });
        }
    }
}

fn extract_signature_to_body(node: Node, source: &str, body_kind: &str) -> String {
    let sig_node = export_aware_sig_node(node);
    let body = first_child_by_kind(node, body_kind);
    let end_byte = body
        .map(|b| b.start_byte())
        .unwrap_or_else(|| node.end_byte());
    flatten_bytes(sig_node.start_byte(), end_byte, source)
        .unwrap_or_else(|| first_line_of_node(sig_node, source))
}

#[allow(clippy::too_many_arguments)]
fn handle_definition<'a>(
    node: Node<'a>,
    source: &str,
    mode: &MatchMode,
    kinds: &[DefKind],
    results: &mut Vec<DefContent>,
    def_kind: DefKind,
    body_kind: &str,
    scope: &str,
) {
    if !kinds.contains(&def_kind) {
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

    let name = name_ref.to_string();
    let signature = extract_signature_to_body(node, source, body_kind);
    let start_row = node.start_position().row + 1;
    let [start, end] = line_range(start_row, node);
    let def_scope = build_scope(scope, ".", &name);

    results.push(DefContent {
        kind: def_kind,
        lines: [start, end],
        signature,
        scope: def_scope,
    });
}

/// Handle ERROR nodes that may contain incomplete function declarations.
fn handle_error_function<'a>(
    node: Node<'a>,
    source: &str,
    mode: &MatchMode,
    kinds: &[DefKind],
    results: &mut Vec<DefContent>,
    scope: &str,
) {
    if !kinds.contains(&DefKind::Function) {
        return;
    }

    if first_child_by_kind(node, "function").is_none() {
        return;
    }

    let name_node = match first_child_by_kind(node, "identifier") {
        Some(n) => n,
        None => return,
    };
    let name_ref = node_text_ref(name_node, source);

    if !mode.matches_ident(name_ref) {
        return;
    }

    let name = name_ref.to_string();
    let signature = first_line_of_node(node, source);
    let start_row = node.start_position().row + 1;
    let [start, end] = line_range(start_row, node);
    let def_scope = build_scope(scope, ".", &name);

    results.push(DefContent {
        kind: DefKind::Function,
        lines: [start, end],
        signature,
        scope: def_scope,
    });
}

fn recurse_into_body(
    node: Node,
    source: &str,
    mode: &MatchMode,
    kinds: &[DefKind],
    results: &mut Vec<DefContent>,
    scope: &str,
) {
    if let Some(body) = node.child_by_field_name("body") {
        let mut cursor = body.walk();
        for child in body.children(&mut cursor) {
            collect_definitions(child, source, mode, kinds, results, scope);
        }
    }
}

fn unwrap_first_named_child(node: Node) -> Option<Node> {
    let child = node.named_child(0)?;
    if child.kind() == "parenthesized_expression" {
        child.named_child(0)
    } else {
        Some(child)
    }
}

fn handle_lexical_decl<'a>(
    node: Node<'a>,
    source: &str,
    mode: &MatchMode,
    kinds: &[DefKind],
    results: &mut Vec<DefContent>,
    scope: &str,
) {
    let is_const = first_child_by_kind(node, "const").is_some();

    let sig_start_node = export_aware_sig_node(node);

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() != "variable_declarator" {
            continue;
        }

        let name_node = match child.child_by_field_name("name") {
            Some(n) => n,
            None => continue,
        };
        let name_ref = node_text_ref(name_node, source);

        let value_node = child.child_by_field_name("value");
        let effective_value = value_node.and_then(|v| match v.kind() {
            "satisfies_expression" | "as_expression" => unwrap_first_named_child(v),
            _ => Some(v),
        });
        let def_kind = match effective_value.map(|v| v.kind()) {
            Some("arrow_function") | Some("function_expression") => DefKind::Function,
            Some("class") => DefKind::Class,
            _ => {
                if is_const {
                    DefKind::Const
                } else {
                    DefKind::Var
                }
            }
        };

        let own_scope = build_scope(scope, ".", name_ref);

        if kinds.contains(&def_kind) && mode.matches_ident(name_ref) {
            let signature = flatten_bytes(sig_start_node.start_byte(), child.end_byte(), source)
                .unwrap_or_else(|| first_line_of_node(sig_start_node, source));
            let start_row = sig_start_node.start_position().row + 1;
            let [start, end] = line_range(start_row, node);

            results.push(DefContent {
                kind: def_kind,
                lines: [start, end],
                signature,
                scope: own_scope.clone(),
            });
        }

        // Recurse into value body for class/object definitions (not function bodies)
        if let Some(value) = effective_value {
            match value.kind() {
                "class" => {
                    if let Some(body) = value.child_by_field_name("body") {
                        let mut bc = body.walk();
                        for gc in body.children(&mut bc) {
                            collect_definitions(gc, source, mode, kinds, results, &own_scope);
                        }
                    }
                }
                "object" => {
                    let mut vc = value.walk();
                    for gc in value.children(&mut vc) {
                        collect_definitions(gc, source, mode, kinds, results, &own_scope);
                    }
                }
                _ => {}
            }
        }
    }
}

/// Handle a public_field_definition node (class field in TS).
fn handle_field<'a>(
    node: Node<'a>,
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
    let signature = first_line_of_node(node, source);
    let start_row = node.start_position().row + 1;
    let [start, end] = line_range(start_row, node);

    results.push(DefContent {
        kind: DefKind::Field,
        lines: [start, end],
        signature,
        scope: own_scope,
    });
}

/// Handle a property_signature node (interface property in TS).
fn handle_property_signature<'a>(
    node: Node<'a>,
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
    let signature = first_line_of_node(node, source);
    let start_row = node.start_position().row + 1;
    let [start, end] = line_range(start_row, node);

    results.push(DefContent {
        kind: DefKind::PropertyDeclaration,
        lines: [start, end],
        signature,
        scope: own_scope,
    });
}

fn handle_type_alias<'a>(
    node: Node<'a>,
    source: &str,
    mode: &MatchMode,
    kinds: &[DefKind],
    results: &mut Vec<DefContent>,
    scope: &str,
) {
    if !kinds.contains(&DefKind::Alias) {
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

    let name = name_ref.to_string();
    let sig_node = export_aware_sig_node(node);
    let signature = flatten_bytes(sig_node.start_byte(), node.end_byte(), source)
        .unwrap_or_else(|| first_line_of_node(sig_node, source));
    let start_row = sig_node.start_position().row + 1;
    let [start, end] = line_range(start_row, node);
    let def_scope = build_scope(scope, ".", &name);

    results.push(DefContent {
        kind: DefKind::Alias,
        lines: [start, end],
        signature,
        scope: def_scope,
    });
}

/// Handle a property_identifier node inside an enum_body (TS enum member / Variant).
fn handle_enum_variant<'a>(
    node: Node<'a>,
    source: &str,
    mode: &MatchMode,
    results: &mut Vec<DefContent>,
    scope: &str,
) {
    let name_ref = node_text_ref(node, source);
    if !mode.matches_ident(name_ref) {
        return;
    }
    let name = name_ref.to_string();
    let own_scope = build_scope(scope, ".", &name);
    let signature = first_line_of_node(node, source);
    let start_row = node.start_position().row + 1;
    let [start, end] = line_range(start_row, node);

    results.push(DefContent {
        kind: DefKind::Variant,
        lines: [start, end],
        signature,
        scope: own_scope,
    });
}

/// Handle an index_signature node (TS `[key: string]: T` — Subscript kind).
fn handle_index_signature<'a>(
    node: Node<'a>,
    source: &str,
    mode: &MatchMode,
    results: &mut Vec<DefContent>,
    scope: &str,
) {
    // Build display name from the bracket pattern, e.g. "[key: string]"
    let name = extract_index_signature_name(node, source);
    if !mode.matches_ident(&name) {
        return;
    }
    let own_scope = build_scope(scope, ".", &name);
    let signature = first_line_of_node(node, source);
    let start_row = node.start_position().row + 1;
    let [start, end] = line_range(start_row, node);

    results.push(DefContent {
        kind: DefKind::SubscriptDeclaration,
        lines: [start, end],
        signature,
        scope: own_scope,
    });
}

/// Extract the display name from an index_signature node.
/// Returns the bracket pattern like "[key: string]" by finding the
/// text between the opening "[" and closing "]".
fn extract_index_signature_name(node: Node, source: &str) -> String {
    let mut cursor = node.walk();
    let mut start = None;
    let mut end = None;
    for child in node.children(&mut cursor) {
        if child.kind() == "[" {
            start = Some(child.start_byte());
        }
        if child.kind() == "]" {
            end = Some(child.end_byte());
            break;
        }
    }
    match (start, end) {
        (Some(s), Some(e)) => source[s..e].to_string(),
        _ => "[index]".to_string(),
    }
}

/// Handle a construct_signature node (`new(arg: T): RetType` — Constructor kind).
fn handle_construct_signature<'a>(
    node: Node<'a>,
    source: &str,
    mode: &MatchMode,
    results: &mut Vec<DefContent>,
    scope: &str,
) {
    let name = "new";
    if !mode.matches_ident(name) {
        return;
    }
    let own_scope = build_scope(scope, ".", name);
    let signature = first_line_of_node(node, source);
    let start_row = node.start_position().row + 1;
    let [start, end] = line_range(start_row, node);

    results.push(DefContent {
        kind: DefKind::ConstructorDeclaration,
        lines: [start, end],
        signature,
        scope: own_scope,
    });
}

/// Handle a call_signature node (`(arg: T): RetType` — Method kind).
fn handle_call_signature<'a>(
    node: Node<'a>,
    source: &str,
    mode: &MatchMode,
    results: &mut Vec<DefContent>,
    scope: &str,
) {
    let name = "call";
    if !mode.matches_ident(name) {
        return;
    }
    let own_scope = build_scope(scope, ".", name);
    let signature = first_line_of_node(node, source);
    let start_row = node.start_position().row + 1;
    let [start, end] = line_range(start_row, node);

    results.push(DefContent {
        kind: DefKind::MethodDeclaration,
        lines: [start, end],
        signature,
        scope: own_scope,
    });
}

fn collect_definitions<'a>(
    node: Node<'a>,
    source: &str,
    mode: &MatchMode,
    kinds: &[DefKind],
    results: &mut Vec<DefContent>,
    scope: &str,
) {
    match node.kind() {
        "export_statement" => {
            if let Some(decl) = node.child_by_field_name("declaration") {
                collect_definitions(decl, source, mode, kinds, results, scope);
            }
            return;
        }
        "ambient_declaration" => {
            handle_ambient_declaration(node, source, mode, kinds, results, scope);
            return;
        }
        "function_declaration" | "generator_function_declaration" => {
            handle_definition(
                node,
                source,
                mode,
                kinds,
                results,
                DefKind::Function,
                "statement_block",
                scope,
            );
            return;
        }
        "function_signature" => {
            handle_definition(
                node,
                source,
                mode,
                kinds,
                results,
                DefKind::FunctionDeclaration,
                "statement_block",
                scope,
            );
            return;
        }
        "class_declaration" => {
            handle_definition(
                node,
                source,
                mode,
                kinds,
                results,
                DefKind::Class,
                "class_body",
                scope,
            );
            let new_scope = build_scope_from_node(node, source, scope, ".");
            recurse_into_body(node, source, mode, kinds, results, &new_scope);
            return;
        }
        "abstract_class_declaration" => {
            handle_definition(
                node,
                source,
                mode,
                kinds,
                results,
                DefKind::Class,
                "class_body",
                scope,
            );
            let new_scope = build_scope_from_node(node, source, scope, ".");
            recurse_into_body(node, source, mode, kinds, results, &new_scope);
            return;
        }
        "interface_declaration" => {
            handle_definition(
                node,
                source,
                mode,
                kinds,
                results,
                DefKind::Interface,
                "interface_body",
                scope,
            );
            let new_scope = build_scope_from_node(node, source, scope, ".");
            recurse_into_body(node, source, mode, kinds, results, &new_scope);
            return;
        }
        "type_alias_declaration" => {
            handle_type_alias(node, source, mode, kinds, results, scope);
            return;
        }
        "enum_declaration" => {
            handle_definition(
                node,
                source,
                mode,
                kinds,
                results,
                DefKind::Enum,
                "enum_body",
                scope,
            );
            let enum_scope = build_scope_from_node(node, source, scope, ".");
            if let Some(body) = first_child_by_kind(node, "enum_body") {
                let mut cursor = body.walk();
                for child in body.children(&mut cursor) {
                    collect_definitions(child, source, mode, kinds, results, &enum_scope);
                }
            }
            return;
        }
        "lexical_declaration" => {
            handle_lexical_decl(node, source, mode, kinds, results, scope);
            return;
        }
        "variable_declaration" => {
            handle_var_decl(node, source, mode, kinds, results, scope);
            return;
        }
        "method_definition" => {
            let def_kind = classify_method_definition(node, source);
            handle_definition(
                node,
                source,
                mode,
                kinds,
                results,
                def_kind,
                "statement_block",
                scope,
            );
            return;
        }
        "pair" => {
            handle_pair(node, source, mode, kinds, results, scope);
            return;
        }
        "method_signature" | "abstract_method_signature" => {
            let base_kind = classify_method_definition(node, source);
            let def_kind = base_kind.declaration_pair().unwrap_or(base_kind);
            handle_definition(
                node,
                source,
                mode,
                kinds,
                results,
                def_kind,
                "statement_block",
                scope,
            );
            return;
        }
        "internal_module" => {
            let new_scope = build_scope_from_node(node, source, scope, ".");
            if kinds.contains(&DefKind::Namespace) && mode.matches_ident(&new_scope) {
                let sig = extract_signature_to_body(node, source, "statement_block");
                let start_row = node.start_position().row + 1;
                let [start, end] = line_range(start_row, node);
                results.push(DefContent {
                    kind: DefKind::Namespace,
                    lines: [start, end],
                    signature: sig,
                    scope: new_scope.clone(),
                });
            }
            recurse_into_body(node, source, mode, kinds, results, &new_scope);
            return;
        }
        "module" => {
            // Discriminate by name field type: string → ModuleDeclaration, identifier → Namespace
            let name_node = node.child_by_field_name("name");
            let is_ambient = name_node.map(|n| n.kind() == "string").unwrap_or(false);

            if is_ambient {
                // Extract unquoted name from string > string_fragment
                let name = name_node
                    .and_then(|s| first_child_by_kind(s, "string_fragment"))
                    .map(|sf| node_text_ref(sf, source))
                    .unwrap_or("");
                let new_scope = build_scope(scope, ".", name);
                if kinds.contains(&DefKind::ModuleDeclaration) && mode.matches_ident(&new_scope) {
                    let sig = extract_signature_to_body(node, source, "statement_block");
                    let start_row = node.start_position().row + 1;
                    let [start, end] = line_range(start_row, node);
                    results.push(DefContent {
                        kind: DefKind::ModuleDeclaration,
                        lines: [start, end],
                        signature: sig,
                        scope: new_scope.clone(),
                    });
                }
                recurse_into_body(node, source, mode, kinds, results, &new_scope);
            } else {
                let new_scope = build_scope_from_node(node, source, scope, ".");
                if kinds.contains(&DefKind::Namespace) && mode.matches_ident(&new_scope) {
                    let sig = extract_signature_to_body(node, source, "statement_block");
                    let start_row = node.start_position().row + 1;
                    let [start, end] = line_range(start_row, node);
                    results.push(DefContent {
                        kind: DefKind::Namespace,
                        lines: [start, end],
                        signature: sig,
                        scope: new_scope.clone(),
                    });
                }
                recurse_into_body(node, source, mode, kinds, results, &new_scope);
            }
            return;
        }
        "ERROR" => {
            handle_error_function(node, source, mode, kinds, results, scope);
            return;
        }
        "public_field_definition" => {
            if kinds.contains(&DefKind::Field) {
                handle_field(node, source, mode, results, scope);
            }
            return;
        }
        "property_signature" => {
            if kinds.contains(&DefKind::PropertyDeclaration) {
                handle_property_signature(node, source, mode, results, scope);
            }
            return;
        }
        "property_identifier" => {
            // Inside enum_body, property_identifier nodes are enum members (Variant kind).
            // Verify parent is enum_body to avoid extracting other property_identifier nodes.
            if kinds.contains(&DefKind::Variant) {
                if let Some(parent) = node.parent() {
                    if parent.kind() == "enum_body" {
                        handle_enum_variant(node, source, mode, results, scope);
                    }
                }
            }
            return;
        }
        "enum_assignment" => {
            // enum_assignment wraps a property_identifier with an initializer value.
            // Extract the inner property_identifier as a Variant.
            if kinds.contains(&DefKind::Variant) {
                if let Some(name_node) = node.child_by_field_name("name") {
                    handle_enum_variant(name_node, source, mode, results, scope);
                } else {
                    // Fallback: find first property_identifier child
                    if let Some(pi) = first_child_by_kind(node, "property_identifier") {
                        handle_enum_variant(pi, source, mode, results, scope);
                    }
                }
            }
            return;
        }
        "index_signature" => {
            if kinds.contains(&DefKind::SubscriptDeclaration) {
                handle_index_signature(node, source, mode, results, scope);
            }
            return;
        }
        "construct_signature" => {
            if kinds.contains(&DefKind::ConstructorDeclaration) {
                handle_construct_signature(node, source, mode, results, scope);
            }
            return;
        }
        "call_signature" => {
            if kinds.contains(&DefKind::MethodDeclaration) {
                handle_call_signature(node, source, mode, results, scope);
            }
            return;
        }
        _ => {}
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_definitions(child, source, mode, kinds, results, scope);
    }
}
