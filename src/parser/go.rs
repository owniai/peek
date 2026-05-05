use crate::model::{DefContent, DefKind};
use crate::parser::{
    LanguageParser, MatchMode, first_child_by_kind, first_line_of_node, flatten_bytes, line_range,
    node_text, node_text_ref, normalize_signature,
};
use tree_sitter::{Node, Parser};

pub struct GoParser;

impl LanguageParser for GoParser {
    fn language(&self) -> &'static str {
        "go"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &[".go"]
    }

    fn supported_kinds(&self) -> &'static [DefKind] {
        &[
            DefKind::Function,
            DefKind::Method,
            DefKind::Struct,
            DefKind::Interface,
            DefKind::Alias,
            DefKind::Const,
            DefKind::Field,
            DefKind::Package,
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
            let name_node = match child.child_by_field_name("name") {
                Some(n) => n,
                None => continue, // Skip embedded fields without explicit name
            };
            let name_ref = node_text_ref(name_node, source);
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

/// Extract method_elem from an interface_type as Method definitions.
fn handle_interface_methods<'a>(
    iface: Node<'a>,
    iface_name: &str,
    source: &str,
    mode: &MatchMode,
    kinds: &[DefKind],
    results: &mut Vec<DefContent>,
) {
    if !kinds.contains(&DefKind::Method) {
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
                        kind: DefKind::Method,
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

/// Extract signature from node start to the `block` body boundary.
fn extract_signature_to_block(node: Node, source: &str) -> String {
    let body = first_child_by_kind(node, "block");
    let end_byte = body
        .map(|b| b.start_byte())
        .unwrap_or_else(|| node.end_byte());
    flatten_bytes(node.start_byte(), end_byte, source)
        .unwrap_or_else(|| first_line_of_node(node, source))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::extract_definitions;

    // --- Bug: grouped type declaration line range ---

    #[test]
    fn grouped_type_struct_line_range() {
        let src = "type (\n\tGroupedPoint struct {\n\t\tX float64\n\t\tY float64\n\t}\n\tGroupedHandler interface {\n\t\tHandle() error\n\t}\n\tGroupedInt int\n)";
        let results = extract_definitions(&GoParser, "GroupedPoint", &[DefKind::Struct], src);
        assert_eq!(results.len(), 1);
        assert_eq!(
            results[0].lines[0], 2,
            "start line should be 2 (where GroupedPoint is defined), not 1 (where 'type (' is)"
        );
    }

    #[test]
    fn grouped_type_interface_line_range() {
        let src = "type (\n\tGroupedPoint struct {\n\t\tX float64\n\t}\n\tGroupedHandler interface {\n\t\tHandle() error\n\t}\n)";
        let results = extract_definitions(&GoParser, "GroupedHandler", &[DefKind::Interface], src);
        assert_eq!(results.len(), 1);
        assert_eq!(
            results[0].lines[0], 5,
            "start line should be 5 (where GroupedHandler is defined), not 1 (where 'type (' is)"
        );
    }

    #[test]
    fn grouped_type_definition_line_range() {
        let src = "type (\n\tGroupedPoint struct { X float64 }\n\tGroupedInt int\n)";
        let results = extract_definitions(&GoParser, "GroupedInt", &[DefKind::Alias], src);
        assert_eq!(results.len(), 1);
        assert_eq!(
            results[0].lines[0], 3,
            "start line should be 3 (where GroupedInt is defined), not 1 (where 'type (' is)"
        );
    }

    #[test]
    fn grouped_type_no_duplicate_extraction() {
        let src = "package p\n\ntype (\n\tFoo struct { X int }\n\tBar interface { Do() }\n\tAlias int\n)\n\ntype Standalone struct { Y int }";
        let all_kinds = &[DefKind::Struct, DefKind::Interface, DefKind::Alias];
        let results = extract_definitions(&GoParser, "", all_kinds, src);
        let scopes: Vec<&str> = results.iter().map(|r| r.scope.as_str()).collect();
        assert_eq!(
            results.len(),
            4,
            "expected exactly 4 definitions, got {scopes:?}"
        );
    }

    #[test]
    fn grouped_const_no_duplicate_extraction() {
        let src = "package p\n\nconst (\n\tA = 1\n\tB = 2\n\tC = 3\n)\n\nconst D = 4";
        let results = extract_definitions(&GoParser, "", &[DefKind::Const], src);
        let scopes: Vec<&str> = results.iter().map(|r| r.scope.as_str()).collect();
        assert_eq!(
            results.len(),
            4,
            "expected exactly 4 constants, got {scopes:?}"
        );
    }

    // --- Field extraction ---

    #[test]
    fn struct_field_extracted_as_field() {
        let src = "package p\n\ntype Server struct { Host string; Port int }";
        let results = extract_definitions(&GoParser, "Host", &[DefKind::Field], src);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].kind, DefKind::Field);
        assert_eq!(results[0].scope, "Server.Host");
    }

    #[test]
    fn struct_multiple_fields() {
        let src = "package p\n\ntype Point struct { X float64; Y float64 }";
        let results = extract_definitions(&GoParser, "Y", &[DefKind::Field], src);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].kind, DefKind::Field);
        assert_eq!(results[0].scope, "Point.Y");
    }

    #[test]
    fn field_kind_filter_excludes_struct() {
        let src = "package p\n\ntype Foo struct { Bar int }";
        let results = extract_definitions(&GoParser, "Bar", &[DefKind::Struct], src);
        assert!(
            results.is_empty(),
            "field should not match -k struct, got: {results:?}"
        );
    }

    #[test]
    fn embedded_field_without_name_skipped() {
        // Embedded fields in Go have no explicit field name (no field_identifier)
        let src = "package p\n\ntype Embedded struct { Server }";
        let results = extract_definitions(&GoParser, ".", &[DefKind::Field], src);
        // Embedded fields should be skipped (no field_identifier name)
        assert!(
            results.is_empty(),
            "embedded field without name should be skipped, got: {results:?}"
        );
    }
}
