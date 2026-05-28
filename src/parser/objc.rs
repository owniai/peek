use crate::model::{DefContent, DefKind};
use crate::parser::{
    LanguageParser, MatchMode, build_scope, extract_const_name, extract_function_name,
    extract_signature_to_body, extract_typedef_name, extract_var_name, first_child_by_kind,
    first_line_of_node, flatten_bytes, handle_macro, has_extern_storage_class,
    has_function_declarator, has_initializer, is_const_declaration, line_range, node_text,
    node_text_ref, normalize_signature,
};
use tree_sitter::{Node, Parser};

pub(crate) const LANGUAGE: &str = "objc";
pub(crate) const EXTENSIONS: &[&str] = &["m", "h"];
pub(crate) const ALIASES: &[&str] = &["objective-c", "obj-c"];

pub struct ObjCParser;

impl LanguageParser for ObjCParser {
    fn language(&self) -> &'static str {
        LANGUAGE
    }

    fn extensions(&self) -> &'static [&'static str] {
        EXTENSIONS
    }

    fn supported_kinds(&self) -> &'static [DefKind] {
        &[
            DefKind::Class,
            DefKind::ClassDeclaration,
            DefKind::Extension,
            DefKind::Protocol,
            DefKind::Method,
            DefKind::MethodDeclaration,
            DefKind::Property,
            DefKind::Field,
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
            DefKind::Var,
            DefKind::VarDeclaration,
            DefKind::Variant,
        ]
    }

    impl_init_parser!(tree_sitter_objc::LANGUAGE, "ObjC");

    impl_extract_with!(collect_definitions, scope: "");
}

/// Extract class name and optional category from class_interface / class_implementation.
///
/// Returns `(class_name, category)` where category is:
/// - `None` → plain class (no parens)
/// - `Some("")` → empty category `()`
/// - `Some("Cat")` → named category `(Cat)`
fn extract_class_info(node: Node, source: &str) -> (String, Option<String>) {
    let mut cursor = node.walk();
    let children: Vec<Node> = node.children(&mut cursor).collect();

    let class_name = children
        .iter()
        .find(|c| c.kind() == "identifier")
        .map(|c| node_text(*c, source))
        .unwrap_or_default();

    let mut found_paren = false;
    for child in &children {
        if !child.is_named() {
            if let Ok(text) = child.utf8_text(source.as_bytes()) {
                let trimmed = text.trim();
                if trimmed == "(" {
                    found_paren = true;
                    continue;
                }
                if trimmed == ")" && found_paren {
                    return (class_name, Some(String::new()));
                }
            }
        } else if found_paren && child.kind() == "identifier" {
            return (class_name, Some(node_text(*child, source)));
        }
    }

    (class_name, None)
}

/// Extract method name (first identifier) from method_declaration / method_definition.
fn extract_method_name(node: Node, source: &str) -> Option<String> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "identifier" {
            return Some(node_text(child, source));
        }
    }
    None
}

/// Extract method signature: everything from start to compound_statement (body).
fn extract_method_signature(node: Node, source: &str) -> String {
    let end_byte = if let Some(body) = first_child_by_kind(node, "compound_statement") {
        body.start_byte()
    } else {
        node.end_byte()
    };
    let sig = flatten_bytes(node.start_byte(), end_byte, source)
        .unwrap_or_else(|| first_line_of_node(node, source));
    normalize_signature(&sig)
}

/// Find identifier inside struct_declarator (handles pointer_declarator nesting).
fn find_identifier_in(node: Node, source: &str) -> Option<String> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "identifier" => return Some(node_text(child, source)),
            "pointer_declarator" | "struct_declarator" => {
                if let Some(name) = find_identifier_in(child, source) {
                    return Some(name);
                }
            }
            _ => {}
        }
    }
    None
}

/// Extract property/ivar name from the struct_declaration child.
fn extract_struct_declarator_name(parent: Node, source: &str) -> Option<String> {
    let sd = first_child_by_kind(parent, "struct_declarator")?;
    find_identifier_in(sd, source)
}

/// Extract NS_ENUM / NS_OPTIONS name from a type_definition containing macro_type_specifier.
///
/// The enum name is in a direct `type_descriptor → type_identifier` child of macro_type_specifier
/// (not inside the ERROR node that wraps the first macro argument).
fn extract_nsenum_name(type_def: Node, source: &str) -> Option<String> {
    let macro_node = first_child_by_kind(type_def, "macro_type_specifier")?;
    let first_ident = first_child_by_kind(macro_node, "identifier")?;
    let macro_name = node_text_ref(first_ident, source);
    if macro_name != "NS_ENUM" && macro_name != "NS_OPTIONS" {
        return None;
    }

    let mut cursor = macro_node.walk();
    for child in macro_node.children(&mut cursor) {
        if child.kind() == "type_descriptor" {
            if let Some(ti) = first_child_by_kind(child, "type_identifier") {
                return Some(node_text(ti, source));
            }
        }
    }
    None
}

/// Extract class/protocol signature by truncating at the first body-like child.
fn extract_class_signature(node: Node, source: &str) -> String {
    let mut end_byte = node.end_byte();
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if matches!(
            child.kind(),
            "instance_variables"
                | "property_declaration"
                | "method_declaration"
                | "method_definition"
                | "implementation_definition"
        ) {
            end_byte = child.start_byte();
            break;
        }
    }
    let sig = flatten_bytes(node.start_byte(), end_byte, source)
        .unwrap_or_else(|| first_line_of_node(node, source));
    normalize_signature(&sig)
}

fn handle_class_like(
    node: Node,
    source: &str,
    mode: &MatchMode,
    kinds: &[DefKind],
    results: &mut Vec<DefContent>,
    scope: &str,
) {
    let (class_name, category) = extract_class_info(node, source);

    let (def_kind, own_scope) = match &category {
        None => (DefKind::Class, build_scope(scope, ".", &class_name)),
        Some(cat) if cat.is_empty() => (DefKind::Extension, build_scope(scope, ".", &class_name)),
        Some(cat) => {
            let cat_scope = format!("{}.{}", class_name, cat);
            (DefKind::Extension, build_scope(scope, ".", &cat_scope))
        }
    };

    if kinds.contains(&def_kind) && mode.matches_ident(&class_name) {
        let signature = extract_class_signature(node, source);
        let start_row = node.start_position().row + 1;
        let [start, end] = line_range(start_row, node);
        results.push(DefContent {
            kind: def_kind,
            lines: [start, end],
            signature,
            scope: own_scope.clone(),
        });
    }

    recurse_children(node, source, mode, kinds, results, &own_scope);
}

fn handle_protocol(
    node: Node,
    source: &str,
    mode: &MatchMode,
    kinds: &[DefKind],
    results: &mut Vec<DefContent>,
    scope: &str,
) {
    if !kinds.contains(&DefKind::Protocol) {
        return;
    }

    let name_node = match first_child_by_kind(node, "identifier") {
        Some(n) => n,
        None => return,
    };
    let name_ref = node_text_ref(name_node, source);
    let own_scope = build_scope(scope, ".", name_ref);

    if mode.matches_ident(name_ref) {
        let signature = extract_class_signature(node, source);
        let start_row = node.start_position().row + 1;
        let [start, end] = line_range(start_row, node);
        results.push(DefContent {
            kind: DefKind::Protocol,
            lines: [start, end],
            signature,
            scope: own_scope.clone(),
        });
    }

    recurse_children(node, source, mode, kinds, results, &own_scope);
}

fn handle_method(
    node: Node,
    source: &str,
    mode: &MatchMode,
    kinds: &[DefKind],
    results: &mut Vec<DefContent>,
    scope: &str,
    def_kind: DefKind,
) {
    if !kinds.contains(&def_kind) {
        return;
    }

    let name = match extract_method_name(node, source) {
        Some(n) => n,
        None => return,
    };
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

fn handle_struct_decl_node(
    node: Node,
    source: &str,
    mode: &MatchMode,
    kinds: &[DefKind],
    results: &mut Vec<DefContent>,
    scope: &str,
    def_kind: DefKind,
) {
    if !kinds.contains(&def_kind) {
        return;
    }

    let struct_decl = match first_child_by_kind(node, "struct_declaration") {
        Some(sd) => sd,
        None => return,
    };
    let name = match extract_struct_declarator_name(struct_decl, source) {
        Some(n) => n,
        None => return,
    };
    if !mode.matches_ident(&name) {
        return;
    }

    let own_scope = build_scope(scope, ".", &name);
    let signature = first_line_of_node(node, source);
    let start_row = node.start_position().row + 1;
    let [start, end] = line_range(start_row, node);

    results.push(DefContent {
        kind: def_kind,
        lines: [start, end],
        signature,
        scope: own_scope,
    });
}

fn handle_class_fwd_decl(
    node: Node,
    source: &str,
    mode: &MatchMode,
    kinds: &[DefKind],
    results: &mut Vec<DefContent>,
) {
    if !kinds.contains(&DefKind::ClassDeclaration) {
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

    let signature = first_line_of_node(node, source);
    let start_row = node.start_position().row + 1;
    let [start, end] = line_range(start_row, node);

    results.push(DefContent {
        kind: DefKind::ClassDeclaration,
        lines: [start, end],
        signature,
        scope: name_ref.to_string(),
    });
}

// ── C-inherited handlers (adapted from C parser with `.` separator) ──

fn handle_c_function(
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
    if let Some(name) = extract_nsenum_name(node, source) {
        if !kinds.contains(&DefKind::Enum) {
            return;
        }
        if !mode.matches_ident(&name) {
            return;
        }
        let signature = first_line_of_node(node, source);
        let start_row = node.start_position().row + 1;
        let [start, end] = line_range(start_row, node);
        results.push(DefContent {
            kind: DefKind::Enum,
            lines: [start, end],
            signature,
            scope: name,
        });
        return;
    }

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

fn handle_c_field(
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
    let name_text = match extract_c_field_name(declarator, source) {
        Some(n) => n,
        None => return,
    };
    if !mode.matches_ident(&name_text) {
        return;
    }

    let field_scope = build_scope(scope, ".", &name_text);
    let signature = flatten_bytes(node.start_byte(), node.end_byte(), source)
        .unwrap_or_else(|| first_line_of_node(node, source));
    let start_row = node.start_position().row + 1;
    let [start, end] = line_range(start_row, node);
    results.push(DefContent {
        kind: DefKind::Field,
        lines: [start, end],
        signature,
        scope: field_scope,
    });
}

fn extract_c_field_name(declarator: Node, source: &str) -> Option<String> {
    match declarator.kind() {
        "field_identifier" => Some(node_text(declarator, source)),
        "pointer_declarator" => {
            let mut cursor = declarator.walk();
            for child in declarator.children(&mut cursor) {
                if child.kind() == "field_identifier" {
                    return Some(node_text(child, source));
                }
                if child.kind() == "pointer_declarator" {
                    if let Some(name) = extract_c_field_name(child, source) {
                        return Some(name);
                    }
                }
            }
            None
        }
        _ => None,
    }
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

    let variant_scope = build_scope(scope, ".", &name);
    let signature = flatten_bytes(node.start_byte(), node.end_byte(), source)
        .unwrap_or_else(|| first_line_of_node(node, source));
    let start_row = node.start_position().row + 1;
    let [start, end] = line_range(start_row, node);
    results.push(DefContent {
        kind: DefKind::Variant,
        lines: [start, end],
        signature,
        scope: variant_scope,
    });
}

fn build_scope_from_node_name(node: Node, source: &str, parent: &str) -> String {
    let name = node
        .child_by_field_name("name")
        .map(|n| node_text(n, source))
        .unwrap_or_default();
    build_scope(parent, ".", &name)
}

fn collect_definitions(
    node: Node,
    source: &str,
    mode: &MatchMode,
    kinds: &[DefKind],
    results: &mut Vec<DefContent>,
    scope: &str,
) {
    match node.kind() {
        "class_interface" => {
            handle_class_like(node, source, mode, kinds, results, scope);
            return;
        }
        "class_implementation" => {
            handle_class_like(node, source, mode, kinds, results, scope);
            return;
        }
        "protocol_declaration" => {
            handle_protocol(node, source, mode, kinds, results, scope);
            return;
        }
        "method_declaration" => {
            handle_method(
                node,
                source,
                mode,
                kinds,
                results,
                scope,
                DefKind::MethodDeclaration,
            );
            return;
        }
        "method_definition" => {
            handle_method(node, source, mode, kinds, results, scope, DefKind::Method);
            return;
        }
        "implementation_definition" => {
            recurse_children(node, source, mode, kinds, results, scope);
            return;
        }
        "property_declaration" => {
            handle_struct_decl_node(node, source, mode, kinds, results, scope, DefKind::Property);
            return;
        }
        "instance_variable" => {
            handle_struct_decl_node(node, source, mode, kinds, results, scope, DefKind::Field);
            return;
        }
        "class_declaration" => {
            handle_class_fwd_decl(node, source, mode, kinds, results);
            return;
        }
        "function_definition" => {
            handle_c_function(node, source, mode, kinds, results, false);
            return;
        }
        "struct_specifier" => {
            handle_struct_like(node, source, mode, kinds, results, DefKind::Struct);
            if let Some(body) = node.child_by_field_name("body") {
                let field_scope = build_scope_from_node_name(node, source, scope);
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
                let field_scope = build_scope_from_node_name(node, source, scope);
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
                let variant_scope = build_scope_from_node_name(node, source, scope);
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
                            .map(|name| build_scope(scope, ".", &name))
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
                            .map(|name| build_scope(scope, ".", &name))
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
            handle_c_function(node, source, mode, kinds, results, is_decl);
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
        "field_declaration" => {
            handle_c_field(node, source, mode, kinds, results, scope);
        }
        "enumerator" => {
            handle_variant(node, source, mode, kinds, results, scope);
            return;
        }
        _ => {}
    }

    recurse_children(node, source, mode, kinds, results, scope);
}

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
