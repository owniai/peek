use crate::model::{DefContent, DefKind};
use crate::parser::{
    LanguageParser, MatchMode, build_scope, build_scope_from_node, extract_const_name,
    extract_function_name, extract_signature_to_body, extract_typedef_name, extract_var_name,
    first_line_of_node, flatten_bytes, handle_macro, has_extern_storage_class,
    has_function_declarator, has_initializer, is_const_declaration, line_range, node_text,
    node_text_ref,
};
use tree_sitter::{Node, Parser};

pub(crate) const LANGUAGE: &str = "c";
pub(crate) const EXTENSIONS: &[&str] = &["c"];
pub(crate) const ALIASES: &[&str] = &[];

pub struct CParser;

impl LanguageParser for CParser {
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
            DefKind::Struct,
            DefKind::StructDeclaration,
            DefKind::Union,
            DefKind::UnionDeclaration,
            DefKind::Enum,
            DefKind::EnumDeclaration,
            DefKind::Alias,
            DefKind::Const,
            DefKind::ConstDeclaration,
            DefKind::Macro,
            DefKind::Field,
            DefKind::Var,
            DefKind::VarDeclaration,
            DefKind::Variant,
        ]
    }

    impl_init_parser!(tree_sitter_c::LANGUAGE, "C");

    fn extract_with(
        &self,
        mode: &MatchMode,
        kinds: &[DefKind],
        source: &str,
        parser: &mut Parser,
    ) -> Result<Vec<DefContent>, ()> {
        let tree = match parser.parse(source, None) {
            Some(t) => t,
            None => return Err(()),
        };
        let mut results = Vec::new();
        collect_definitions(tree.root_node(), source, mode, kinds, &mut results, "");
        Ok(results)
    }
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
        "function_definition" => {
            handle_function(node, source, mode, kinds, results, false);
            return;
        }
        "field_declaration" => {
            handle_field(node, source, mode, kinds, results, scope);
            // Do NOT return -- field_declaration may wrap nested type definitions
            // (e.g., `struct Inner { ... };` inside a struct body is a field_declaration
            // wrapping a struct_specifier). Fall through to recurse into children.
        }
        "struct_specifier" => {
            handle_struct_like(node, source, mode, kinds, results, DefKind::Struct);
            if let Some(body) = node.child_by_field_name("body") {
                let field_scope = build_scope_from_node(node, source, scope, "::");
                let mut cursor = body.walk();
                for child in body.children(&mut cursor) {
                    collect_definitions(child, source, mode, kinds, results, &field_scope);
                }
            }
            return;
        }
        "union_specifier" => {
            handle_struct_like(node, source, mode, kinds, results, DefKind::Union);
            if let Some(body) = node.child_by_field_name("body") {
                let field_scope = build_scope_from_node(node, source, scope, "::");
                let mut cursor = body.walk();
                for child in body.children(&mut cursor) {
                    collect_definitions(child, source, mode, kinds, results, &field_scope);
                }
            }
            return;
        }
        "enum_specifier" => {
            handle_enum(node, source, mode, kinds, results);
            if let Some(body) = node.child_by_field_name("body") {
                let variant_scope = build_scope_from_node(node, source, scope, "::");
                let mut cursor = body.walk();
                for child in body.children(&mut cursor) {
                    collect_definitions(child, source, mode, kinds, results, &variant_scope);
                }
            }
            return;
        }
        "type_definition" => {
            handle_typedef(node, source, mode, kinds, results);
            if let Some(type_node) = node.child_by_field_name("type") {
                if matches!(type_node.kind(), "struct_specifier" | "union_specifier") {
                    if let Some(body) = type_node.child_by_field_name("body") {
                        let field_scope = node
                            .child_by_field_name("declarator")
                            .and_then(|d| extract_typedef_name(d, source))
                            .map(|name| build_scope(scope, "::", &name))
                            .unwrap_or_else(|| scope.to_string());
                        let mut cursor = body.walk();
                        for child in body.children(&mut cursor) {
                            collect_definitions(child, source, mode, kinds, results, &field_scope);
                        }
                    }
                } else if type_node.kind() == "enum_specifier" {
                    if let Some(body) = type_node.child_by_field_name("body") {
                        let variant_scope = node
                            .child_by_field_name("declarator")
                            .and_then(|d| extract_typedef_name(d, source))
                            .map(|name| build_scope(scope, "::", &name))
                            .unwrap_or_else(|| scope.to_string());
                        let mut cursor = body.walk();
                        for child in body.children(&mut cursor) {
                            collect_definitions(
                                child,
                                source,
                                mode,
                                kinds,
                                results,
                                &variant_scope,
                            );
                        }
                    }
                }
            }
            return;
        }
        "declaration" if is_const_declaration(node, source) => {
            handle_const(node, source, mode, kinds, results);
            return;
        }
        "declaration" if has_function_declarator(node) => {
            let is_decl = node.child_by_field_name("body").is_none();
            handle_function(node, source, mode, kinds, results, is_decl);
            return;
        }
        "declaration" if !has_function_declarator(node) => {
            handle_var(node, source, mode, kinds, results);
            return;
        }
        "preproc_def" | "preproc_function_def" => {
            handle_macro(node, source, mode, kinds, results);
            return;
        }
        "enumerator" => {
            handle_variant(node, source, mode, kinds, results, scope);
            return;
        }
        _ => {}
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_definitions(child, source, mode, kinds, results, scope);
    }
}

fn handle_function(
    node: Node,
    source: &str,
    mode: &MatchMode,
    kinds: &[DefKind],
    results: &mut Vec<DefContent>,
    is_declaration: bool,
) {
    let def_kind = if is_declaration {
        DefKind::FunctionDeclaration
    } else {
        DefKind::Function
    };

    if !kinds.contains(&def_kind) {
        return;
    }

    let declarator = match node.child_by_field_name("declarator") {
        Some(d) => d,
        None => return,
    };
    let name_text = match extract_function_name(declarator, source) {
        Some(n) => n,
        None => return,
    };
    if !mode.matches_ident(&name_text) {
        return;
    }

    let signature = extract_signature_to_body(node, source);
    let start_row = node.start_position().row + 1;
    let [start, end] = line_range(start_row, node);
    results.push(DefContent {
        kind: def_kind,
        lines: [start, end],
        signature,
        scope: name_text,
    });
}

fn handle_struct_like(
    node: Node,
    source: &str,
    mode: &MatchMode,
    kinds: &[DefKind],
    results: &mut Vec<DefContent>,
    def_kind: DefKind,
) {
    let has_body = node.child_by_field_name("body").is_some();
    let kind = if has_body {
        if !kinds.contains(&def_kind) {
            return;
        }
        def_kind
    } else {
        let Some(decl_kind) = def_kind.declaration_pair() else {
            return;
        };
        if !kinds.contains(&decl_kind) {
            return;
        }
        decl_kind
    };

    let name_node = match node.child_by_field_name("name") {
        Some(n) => n,
        None => return,
    };
    let name_ref = node_text_ref(name_node, source);
    if !mode.matches_ident(name_ref) {
        return;
    }
    let name = name_ref.to_string();

    let signature = extract_signature_to_body(node, source);
    let start_row = node.start_position().row + 1;
    let [start, end] = line_range(start_row, node);
    results.push(DefContent {
        kind,
        lines: [start, end],
        signature,
        scope: name,
    });
}

fn handle_enum(
    node: Node,
    source: &str,
    mode: &MatchMode,
    kinds: &[DefKind],
    results: &mut Vec<DefContent>,
) {
    let has_body = node.child_by_field_name("body").is_some();
    let kind = if has_body {
        if !kinds.contains(&DefKind::Enum) {
            return;
        }
        DefKind::Enum
    } else {
        if !kinds.contains(&DefKind::EnumDeclaration) {
            return;
        }
        DefKind::EnumDeclaration
    };

    let name_node = match node.child_by_field_name("name") {
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
    results.push(DefContent {
        kind,
        lines: [start, end],
        signature,
        scope: name,
    });
}

fn handle_typedef(
    node: Node,
    source: &str,
    mode: &MatchMode,
    kinds: &[DefKind],
    results: &mut Vec<DefContent>,
) {
    let kind = resolve_typedef_kind(node);

    if !kinds.contains(&kind) {
        return;
    }

    let declarator = match node.child_by_field_name("declarator") {
        Some(d) => d,
        None => return,
    };

    let name_text = match extract_typedef_name(declarator, source) {
        Some(n) => n,
        None => return,
    };
    if !mode.matches_ident(&name_text) {
        return;
    }

    let signature = first_line_of_node(node, source);
    let start_row = node.start_position().row + 1;
    let [start, end] = line_range(start_row, node);
    results.push(DefContent {
        kind,
        lines: [start, end],
        signature,
        scope: name_text,
    });
}

fn resolve_typedef_kind(node: Node) -> DefKind {
    if let Some(type_node) = node.child_by_field_name("type") {
        if type_node.child_by_field_name("body").is_some() {
            return match type_node.kind() {
                "struct_specifier" => DefKind::Struct,
                "union_specifier" => DefKind::Union,
                "enum_specifier" => DefKind::Enum,
                _ => DefKind::Alias,
            };
        }
    }
    DefKind::Alias
}

fn handle_const(
    node: Node,
    source: &str,
    mode: &MatchMode,
    kinds: &[DefKind],
    results: &mut Vec<DefContent>,
) {
    let is_extern_decl = has_extern_storage_class(node, source) && !has_initializer(node);
    let kind = if is_extern_decl {
        if !kinds.contains(&DefKind::ConstDeclaration) {
            return;
        }
        DefKind::ConstDeclaration
    } else {
        if !kinds.contains(&DefKind::Const) {
            return;
        }
        DefKind::Const
    };

    let declarator = match node.child_by_field_name("declarator") {
        Some(d) => d,
        None => return,
    };
    let name_text = match extract_const_name(declarator, source) {
        Some(n) => n,
        None => return,
    };
    if !mode.matches_ident(&name_text) {
        return;
    }

    let signature = first_line_of_node(node, source);
    let start_row = node.start_position().row + 1;
    let [start, end] = line_range(start_row, node);
    results.push(DefContent {
        kind,
        lines: [start, end],
        signature,
        scope: name_text,
    });
}

fn handle_var(
    node: Node,
    source: &str,
    mode: &MatchMode,
    kinds: &[DefKind],
    results: &mut Vec<DefContent>,
) {
    let is_extern_decl = has_extern_storage_class(node, source) && !has_initializer(node);
    let kind = if is_extern_decl {
        if !kinds.contains(&DefKind::VarDeclaration) {
            return;
        }
        DefKind::VarDeclaration
    } else {
        if !kinds.contains(&DefKind::Var) {
            return;
        }
        DefKind::Var
    };

    let declarator = match node.child_by_field_name("declarator") {
        Some(d) => d,
        None => return,
    };
    let name_text = match extract_var_name(declarator, source) {
        Some(n) => n,
        None => return,
    };
    if !mode.matches_ident(&name_text) {
        return;
    }

    let signature = first_line_of_node(node, source);
    let start_row = node.start_position().row + 1;
    let [start, end] = line_range(start_row, node);
    results.push(DefContent {
        kind,
        lines: [start, end],
        signature,
        scope: name_text,
    });
}

fn handle_field(
    node: Node,
    source: &str,
    mode: &MatchMode,
    kinds: &[DefKind],
    results: &mut Vec<DefContent>,
    scope: &str,
) {
    if !kinds.contains(&DefKind::Field) {
        return;
    }

    let declarator = match node.child_by_field_name("declarator") {
        Some(d) => d,
        None => return,
    };
    let name_text = match extract_field_name(declarator, source) {
        Some(n) => n,
        None => return,
    };
    if !mode.matches_ident(&name_text) {
        return;
    }

    let signature = flatten_bytes(node.start_byte(), node.end_byte(), source)
        .unwrap_or_else(|| first_line_of_node(node, source));
    let field_scope = build_scope(scope, "::", &name_text);
    let start_row = node.start_position().row + 1;
    let [start, end] = line_range(start_row, node);
    results.push(DefContent {
        kind: DefKind::Field,
        lines: [start, end],
        signature,
        scope: field_scope,
    });
}

fn handle_variant(
    node: Node,
    source: &str,
    mode: &MatchMode,
    kinds: &[DefKind],
    results: &mut Vec<DefContent>,
    scope: &str,
) {
    if !kinds.contains(&DefKind::Variant) {
        return;
    }

    // enumerator node: try "name" field first, fallback to first identifier child
    let name_node = node.child_by_field_name("name").or_else(|| {
        let mut cursor = node.walk();
        node.children(&mut cursor)
            .find(|c| c.kind() == "identifier")
    });
    let name_node = match name_node {
        Some(n) => n,
        None => return,
    };
    let name_ref = node_text_ref(name_node, source);
    if !mode.matches_ident(name_ref) {
        return;
    }
    let name = name_ref.to_string();

    let signature = flatten_bytes(node.start_byte(), node.end_byte(), source)
        .unwrap_or_else(|| first_line_of_node(node, source));
    let variant_scope = build_scope(scope, "::", &name);
    let start_row = node.start_position().row + 1;
    let [start, end] = line_range(start_row, node);
    results.push(DefContent {
        kind: DefKind::Variant,
        lines: [start, end],
        signature,
        scope: variant_scope,
    });
}

/// Extract field name from a declarator node. Handles both direct field_identifier
/// and pointer_declarator wrapping field_identifier.
fn extract_field_name(declarator: Node, source: &str) -> Option<String> {
    match declarator.kind() {
        "field_identifier" => Some(node_text(declarator, source)),
        "pointer_declarator" => {
            let mut cursor = declarator.walk();
            for child in declarator.children(&mut cursor) {
                if child.kind() == "field_identifier" {
                    return Some(node_text(child, source));
                }
                // Handle nested pointer_declarator (e.g., **name)
                if child.kind() == "pointer_declarator" {
                    if let Some(name) = extract_field_name(child, source) {
                        return Some(name);
                    }
                }
            }
            None
        }
        _ => None,
    }
}
