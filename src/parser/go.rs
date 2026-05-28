use crate::model::{DefContent, DefKind};
use crate::parser::{
    LanguageParser, MatchMode, first_child_by_kind, first_line_of_node, flatten_bytes, line_range,
    node_text, node_text_ref, normalize_signature,
};
use tree_sitter::{Node, Parser};

pub(crate) const LANGUAGE: &str = "go";
pub(crate) const EXTENSIONS: &[&str] = &["go"];
pub(crate) const ALIASES: &[&str] = &["golang"];

pub struct GoParser;

impl LanguageParser for GoParser {
    fn language(&self) -> &'static str {
        LANGUAGE
    }

    fn extensions(&self) -> &'static [&'static str] {
        EXTENSIONS
    }

    fn supported_kinds(&self) -> &'static [DefKind] {
        &[
            DefKind::Function,
            DefKind::Method,
            DefKind::MethodDeclaration,
            DefKind::Struct,
            DefKind::Interface,
            DefKind::Alias,
            DefKind::Const,
            DefKind::Field,
            DefKind::Package,
            DefKind::Var,
        ]
    }

    impl_init_parser!(tree_sitter_go::LANGUAGE, "Go");

    impl_extract_with!(collect_definitions);
}

fn collect_definitions<'a>(
    node: Node<'a>,
    source: &str,
    mode: &MatchMode,
    kinds: &[DefKind],
    results: &mut Vec<DefContent>,
) {
    match node.kind() {
        "package_clause" => {
            if kinds.contains(&DefKind::Package) {
                if let Some(ident) = first_child_by_kind(node, "package_identifier") {
                    let name = node_text(ident, source);
                    if mode.matches_ident(&name) {
                        let signature = normalize_signature(&first_line_of_node(node, source));
                        results.push(DefContent {
                            kind: DefKind::Package,
                            lines: line_range(node.start_position().row + 1, node),
                            signature,
                            scope: name,
                        });
                    }
                }
            }
            return;
        }
        "function_declaration" => {
            handle_function(node, source, mode, kinds, results);
            return;
        }
        "method_declaration" => {
            handle_method(node, source, mode, kinds, results);
            return;
        }
        "type_declaration" => {
            handle_type_declaration(node, source, mode, kinds, results);
            return;
        }
        "const_declaration" => {
            handle_const_declaration(node, source, mode, kinds, results);
            return;
        }
        "var_declaration" => {
            handle_var_declaration(node, source, mode, kinds, results);
            return;
        }
        _ => {}
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_definitions(child, source, mode, kinds, results);
    }
}

fn handle_function<'a>(
    node: Node<'a>,
    source: &str,
    mode: &MatchMode,
    kinds: &[DefKind],
    results: &mut Vec<DefContent>,
) {
    if !kinds.contains(&DefKind::Function) {
        return;
    }

    let name_node = node.child_by_field_name("name");
    if let Some(name_node) = name_node {
        let name_ref = node_text_ref(name_node, source);
        if mode.matches_ident(name_ref) {
            let name = name_ref.to_string();
            let signature = extract_signature_to_block(node, source);
            let start_row = node.start_position().row + 1;
            let [start, end] = line_range(start_row, node);
            results.push(DefContent {
                kind: DefKind::Function,
                lines: [start, end],
                signature,
                scope: name,
            });
        }
    }
}

fn handle_method<'a>(
    node: Node<'a>,
    source: &str,
    mode: &MatchMode,
    kinds: &[DefKind],
    results: &mut Vec<DefContent>,
) {
    if !kinds.contains(&DefKind::Method) {
        return;
    }

    let name_node = node.child_by_field_name("name");
    if let Some(name_node) = name_node {
        let name_ref = node_text_ref(name_node, source);
        if mode.matches_ident(name_ref) {
            let name = name_ref.to_string();
            let receiver_type = extract_receiver_type(node, source);
            let scope = if let Some(rt) = &receiver_type {
                format!("{}.{}", rt, name)
            } else {
                name
            };
            let signature = extract_signature_to_block(node, source);
            let start_row = node.start_position().row + 1;
            let [start, end] = line_range(start_row, node);
            results.push(DefContent {
                kind: DefKind::Method,
                lines: [start, end],
                signature,
                scope,
            });
        }
    }
}

/// Extract receiver type name from method_declaration, stripping generics.
///
/// Paths through: receiver > parameter_list > parameter_declaration > type
/// Handles: type_identifier, pointer_type > type_identifier,
///          pointer_type > generic_type > type_identifier
fn extract_receiver_type(node: Node, source: &str) -> Option<String> {
    let receiver = node.child_by_field_name("receiver")?;
    let param_decl = first_child_by_kind(receiver, "parameter_declaration")?;
    let type_node = param_decl.child_by_field_name("type")?;
    resolve_type_name(type_node, source)
}

/// Resolve a type node to its base type name, stripping pointer and generic wrappers.
fn resolve_type_name(type_node: Node, source: &str) -> Option<String> {
    match type_node.kind() {
        "type_identifier" => Some(node_text(type_node, source)),
        "pointer_type" => {
            let inner = first_child_by_kind(type_node, "type_identifier")
                .or_else(|| first_child_by_kind(type_node, "generic_type"));
            if let Some(inner) = inner {
                resolve_type_name(inner, source)
            } else {
                // Walk children looking for type nodes
                let mut cursor = type_node.walk();
                for child in type_node.children(&mut cursor) {
                    if let Some(name) = resolve_type_name(child, source) {
                        return Some(name);
                    }
                }
                None
            }
        }
        "generic_type" => {
            first_child_by_kind(type_node, "type_identifier").map(|n| node_text(n, source))
        }
        _ => None,
    }
}

fn handle_type_declaration<'a>(
    node: Node<'a>,
    source: &str,
    mode: &MatchMode,
    kinds: &[DefKind],
    results: &mut Vec<DefContent>,
) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "type_spec" => handle_type_spec(node, child, source, mode, kinds, results),
            "type_alias" => handle_type_alias(node, child, source, mode, kinds, results),
            _ => {}
        }
    }
}

fn handle_type_spec<'a>(
    _type_decl: Node<'a>,
    type_spec: Node<'a>,
    source: &str,
    mode: &MatchMode,
    kinds: &[DefKind],
    results: &mut Vec<DefContent>,
) {
    let name_node = type_spec.child_by_field_name("name");
    let type_field = type_spec.child_by_field_name("type");

    let (def_kind, name_ref) = match (&name_node, &type_field) {
        (Some(nn), Some(tf)) => {
            let name = node_text_ref(*nn, source);
            let kind = match tf.kind() {
                "struct_type" => DefKind::Struct,
                "interface_type" => DefKind::Interface,
                _ => DefKind::Alias,
            };
            (kind, name)
        }
        _ => return,
    };

    if !kinds.contains(&def_kind) {
        extract_type_children(&type_field, name_ref, source, mode, kinds, results);
        return;
    }

    if !mode.matches_ident(name_ref) {
        extract_type_children(&type_field, name_ref, source, mode, kinds, results);
        return;
    }

    let signature = match type_field.as_ref().map(|tf| tf.kind()).unwrap_or("") {
        "struct_type" => {
            let body = first_child_by_kind(*type_field.as_ref().unwrap(), "field_declaration_list");
            let end_byte = body
                .map(|b| b.start_byte())
                .unwrap_or_else(|| type_spec.end_byte());
            flatten_bytes(type_spec.start_byte(), end_byte, source)
                .unwrap_or_else(|| first_line_of_node(type_spec, source))
        }
        "interface_type" => {
            let iface = type_field.as_ref().unwrap();
            let end_byte = find_opening_brace(*iface);
            flatten_bytes(type_spec.start_byte(), end_byte, source)
                .unwrap_or_else(|| first_line_of_node(type_spec, source))
        }
        _ => first_line_of_node(type_spec, source),
    };

    let start_row = type_spec.start_position().row + 1;
    let [start, end] = line_range(start_row, type_spec);

    let name = name_ref.to_string();
    results.push(DefContent {
        kind: def_kind,
        lines: [start, end],
        signature,
        scope: name,
    });

    extract_type_children(&type_field, name_ref, source, mode, kinds, results);
}

fn handle_type_alias<'a>(
    type_decl: Node<'a>,
    type_alias: Node<'a>,
    source: &str,
    mode: &MatchMode,
    kinds: &[DefKind],
    results: &mut Vec<DefContent>,
) {
    if !kinds.contains(&DefKind::Alias) {
        return;
    }

    let name_node = type_alias.child_by_field_name("name");
    if let Some(nn) = name_node {
        let name_ref = node_text_ref(nn, source);
        if mode.matches_ident(name_ref) {
            let name = name_ref.to_string();
            let signature = first_line_of_node(type_decl, source);
            let start_row = type_alias.start_position().row + 1;
            let [start, end] = line_range(start_row, type_alias);
            results.push(DefContent {
                kind: DefKind::Alias,
                lines: [start, end],
                signature,
                scope: name,
            });
        }
    }
}

/// Extract struct fields and/or interface methods from a type_spec's type field.
///
/// Called once per handle_type_spec invocation, regardless of which branch is taken,
/// to avoid duplicating child extraction logic across early-return paths.
fn extract_type_children<'a>(
    type_field: &Option<Node<'a>>,
    name_ref: &str,
    source: &str,
    mode: &MatchMode,
    kinds: &[DefKind],
    results: &mut Vec<DefContent>,
) {
    if let Some(tf) = type_field {
        match tf.kind() {
            "struct_type" => handle_struct_fields(*tf, name_ref, source, mode, kinds, results),
            "interface_type" => {
                handle_interface_methods(*tf, name_ref, source, mode, kinds, results)
            }
            _ => {}
        }
    }
}

/// Extract field_declaration nodes from a struct_type as Field definitions.
fn handle_struct_fields<'a>(
    struct_type: Node<'a>,
    struct_name: &str,
    source: &str,
    mode: &MatchMode,
    kinds: &[DefKind],
    results: &mut Vec<DefContent>,
) {
    if !kinds.contains(&DefKind::Field) {
        return;
    }

    let body = match first_child_by_kind(struct_type, "field_declaration_list") {
        Some(b) => b,
        None => return,
    };

    let mut cursor = body.walk();
    for child in body.children(&mut cursor) {
        if child.kind() == "field_declaration" {
            // Regular field has "name" field (field_identifier); embedded field has only "type"
            let name_ref = if let Some(n) = child.child_by_field_name("name") {
                node_text_ref(n, source)
            } else {
                // Embedded field: extract name from type node
                let type_node = match child.child_by_field_name("type") {
                    Some(t) => t,
                    None => continue,
                };
                match type_node.kind() {
                    "type_identifier" => node_text_ref(type_node, source),
                    "qualified_type" => {
                        // *http.Server or http.Server → extract the "Server" part
                        match type_node.child_by_field_name("name") {
                            Some(n) => node_text_ref(n, source),
                            None => continue,
                        }
                    }
                    _ => continue,
                }
            };
            if mode.matches_ident(name_ref) {
                let name = name_ref.to_string();
                let scope = format!("{}.{}", struct_name, name);
                let signature = node_text(child, source);
                let start_row = child.start_position().row + 1;
                let [start, end] = line_range(start_row, child);
                results.push(DefContent {
                    kind: DefKind::Field,
                    lines: [start, end],
                    signature,
                    scope,
                });
            }
        }
    }
}

/// Find the byte position of the opening `{` in an interface_type node.
fn find_opening_brace(iface: Node) -> usize {
    let mut cursor = iface.walk();
    for child in iface.children(&mut cursor) {
        // The opening brace token
        if child.kind() == "{" {
            return child.start_byte();
        }
    }
    iface.end_byte()
}

/// Extract method_elem from an interface_type as MethodDeclaration definitions.
///
/// Interface methods are always declarations (no body), so they use MethodDeclaration
/// instead of Method. This allows `-k method_declaration` to match only interface methods,
/// while `-k method` expands to include both Method and MethodDeclaration.
fn handle_interface_methods<'a>(
    iface: Node<'a>,
    iface_name: &str,
    source: &str,
    mode: &MatchMode,
    kinds: &[DefKind],
    results: &mut Vec<DefContent>,
) {
    // Interface methods are declarations; check for both Method (expanded by kinds_from_tag)
    // and MethodDeclaration to handle -k method and -k method_declaration filters.
    if !kinds.contains(&DefKind::Method) && !kinds.contains(&DefKind::MethodDeclaration) {
        return;
    }

    let mut cursor = iface.walk();
    for child in iface.children(&mut cursor) {
        if child.kind() == "method_elem" {
            let name_node = child.child_by_field_name("name");
            if let Some(nn) = name_node {
                let name_ref = node_text_ref(nn, source);
                if mode.matches_ident(name_ref) {
                    let name = name_ref.to_string();
                    let scope = format!("{}.{}", iface_name, name);
                    let signature = node_text(child, source);
                    let start_row = child.start_position().row + 1;
                    let [start, end] = line_range(start_row, child);
                    results.push(DefContent {
                        kind: DefKind::MethodDeclaration,
                        lines: [start, end],
                        signature,
                        scope,
                    });
                }
            }
        }
    }
}

fn handle_const_declaration<'a>(
    node: Node<'a>,
    source: &str,
    mode: &MatchMode,
    kinds: &[DefKind],
    results: &mut Vec<DefContent>,
) {
    if !kinds.contains(&DefKind::Const) {
        return;
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "const_spec" {
            handle_const_spec(child, source, mode, results);
        }
    }
}

fn handle_const_spec<'a>(
    node: Node<'a>,
    source: &str,
    mode: &MatchMode,
    results: &mut Vec<DefContent>,
) {
    let mut cursor = node.walk();
    for name_node in node.children_by_field_name("name", &mut cursor) {
        let name_ref = node_text_ref(name_node, source);
        if mode.matches_ident(name_ref) {
            let name = name_ref.to_string();
            let signature = node_text(node, source);
            let start_row = node.start_position().row + 1;
            let [start, end] = line_range(start_row, node);
            results.push(DefContent {
                kind: DefKind::Const,
                lines: [start, end],
                signature,
                scope: name,
            });
        }
    }
}

fn handle_var_declaration<'a>(
    node: Node<'a>,
    source: &str,
    mode: &MatchMode,
    kinds: &[DefKind],
    results: &mut Vec<DefContent>,
) {
    if !kinds.contains(&DefKind::Var) {
        return;
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "var_spec" {
            handle_var_spec(child, source, mode, results);
        } else if child.kind() == "var_spec_list" {
            let mut inner = child.walk();
            for spec in child.children(&mut inner) {
                if spec.kind() == "var_spec" {
                    handle_var_spec(spec, source, mode, results);
                }
            }
        }
    }
}

fn handle_var_spec<'a>(
    node: Node<'a>,
    source: &str,
    mode: &MatchMode,
    results: &mut Vec<DefContent>,
) {
    let mut cursor = node.walk();
    for name_node in node.children_by_field_name("name", &mut cursor) {
        let name_ref = node_text_ref(name_node, source);
        if mode.matches_ident(name_ref) {
            let name = name_ref.to_string();
            let signature = node_text(node, source);
            let start_row = node.start_position().row + 1;
            let [start, end] = line_range(start_row, node);
            results.push(DefContent {
                kind: DefKind::Var,
                lines: [start, end],
                signature,
                scope: name,
            });
        }
    }
}

/// Extract signature from node start to the `block` body boundary.
fn extract_signature_to_block(node: Node, source: &str) -> String {
    let body = first_child_by_kind(node, "block");
    let end_byte = body
        .map(|b| b.start_byte())
        .unwrap_or_else(|| node.end_byte());
    flatten_bytes(node.start_byte(), end_byte, source)
        .unwrap_or_else(|| first_line_of_node(node, source))
}
