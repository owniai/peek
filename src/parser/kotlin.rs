use crate::model::{DefContent, DefKind};
use crate::parser::{
    LanguageParser, MatchMode, build_scope, build_scope_from_node, first_child_by_kind,
    first_line_of_node, flatten_bytes, line_range, node_text_ref, normalize_signature,
};
use tree_sitter::{Node, Parser};

pub struct KotlinParser;

impl LanguageParser for KotlinParser {
    fn language(&self) -> &'static str {
        "kotlin"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &[".kt", ".kts"]
    }

    fn supported_kinds(&self) -> &'static [DefKind] {
        &[
            DefKind::Class,
            DefKind::Interface,
            DefKind::Enum,
            DefKind::Function,
            DefKind::Object,
            DefKind::Type,
            DefKind::Const,
        ]
    }

    impl_init_parser!(tree_sitter_kotlin_ng::LANGUAGE, "Kotlin");

    impl_extract_with!(collect_definitions, scope: "");
}

/// Classify a `class_declaration` node as Class, Interface, or Enum.
///
/// Classification logic (based on experiments 2/5/8):
/// - Check anonymous children for "interface" keyword -> Interface
/// - Check modifiers -> class_modifier for "enum" -> Enum
/// - Everything else -> Class (including data/sealed/annotation/abstract/open/inner)
fn classify_class_declaration(node: Node, source: &str) -> DefKind {
    // Check for "interface" keyword among anonymous children
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if !child.is_named() {
            if let Ok(text) = child.utf8_text(source.as_bytes()) {
                if text == "interface" {
                    return DefKind::Interface;
                }
            }
        }
    }

    // Check modifiers for "enum" class_modifier
    if let Some(modifiers) = first_child_by_kind(node, "modifiers") {
        let mut mod_cursor = modifiers.walk();
        for mod_child in modifiers.children(&mut mod_cursor) {
            if mod_child.kind() == "class_modifier" {
                if let Ok(text) = mod_child.utf8_text(source.as_bytes()) {
                    if text == "enum" {
                        return DefKind::Enum;
                    }
                }
            }
        }
    }

    DefKind::Class
}

/// Handle a class_declaration node: extract the definition and recurse into body.
///
/// Works for Class, Interface, and Enum (after classification).
/// Signature: flatten from node start to body boundary, fallback to first line.
/// Body node types: `class_body` (class/interface/object) or `enum_class_body` (enum).
fn handle_class(
    node: Node,
    source: &str,
    mode: &MatchMode,
    def_kind: DefKind,
    kinds: &[DefKind],
    results: &mut Vec<DefContent>,
    scope: &str,
) {
    let body = node.child_by_field_name("body").or_else(|| {
        let mut cursor = node.walk();
        node.children(&mut cursor)
            .find(|c| c.kind() == "class_body" || c.kind() == "enum_class_body")
    });

    let own_scope = build_scope_from_node(node, source, scope, ".");
    if kinds.contains(&def_kind) {
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
                kind: def_kind,
                lines: [start, end],
                signature,
                scope: own_scope.clone(),
            });
        }
    }

    // Always recurse into body to discover nested types
    if let Some(body) = body {
        recurse_children(body, source, mode, kinds, results, &own_scope);
    }
}

/// Handle an object_declaration or companion_object node.
///
/// Skips anonymous companion objects (no name field).
/// Signature: flatten from node start to class_body boundary.
fn handle_object(
    node: Node,
    source: &str,
    mode: &MatchMode,
    kinds: &[DefKind],
    results: &mut Vec<DefContent>,
    scope: &str,
) {
    let name_node = node.child_by_field_name("name");

    // Unnamed companion object: skip the Object definition itself but recurse into body
    let own_scope = match name_node {
        Some(n) => {
            let name_ref = node_text_ref(n, source);
            let own_scope = build_scope_from_node(node, source, scope, ".");

            if kinds.contains(&DefKind::Object) && mode.matches_ident(name_ref) {
                let body = first_child_by_kind(node, "class_body");
                let sig = match body {
                    Some(b) => flatten_bytes(node.start_byte(), b.start_byte(), source)
                        .unwrap_or_else(|| first_line_of_node(node, source)),
                    None => first_line_of_node(node, source),
                };
                let signature = normalize_signature(&sig);
                let start_row = node.start_position().row + 1;
                let [start, end] = line_range(start_row, node);

                results.push(DefContent {
                    kind: DefKind::Object,
                    lines: [start, end],
                    signature,
                    scope: own_scope.clone(),
                });
            }

            own_scope
        }
        None => {
            // Kotlin convention: unnamed companion object has implicit name "Companion"
            build_scope(scope, ".", "Companion")
        }
    };

    // Always recurse into body to discover nested types
    let body = first_child_by_kind(node, "class_body");
    if let Some(body) = body {
        recurse_children(body, source, mode, kinds, results, &own_scope);
    }
}

/// Handle a function_declaration node.
///
/// Signature: truncate to function_body boundary if present, otherwise to
/// function_value_parameters end boundary (for abstract/interface functions).
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

    let own_scope = build_scope_from_node(node, source, scope, ".");
    let raw_sig = if let Some(body) = first_child_by_kind(node, "function_body") {
        flatten_bytes(node.start_byte(), body.start_byte(), source)
            .unwrap_or_else(|| first_line_of_node(node, source))
    } else if let Some(params) = first_child_by_kind(node, "function_value_parameters") {
        flatten_bytes(node.start_byte(), params.end_byte(), source)
            .unwrap_or_else(|| first_line_of_node(node, source))
    } else {
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

/// Handle a type_alias node.
///
/// Note: name is in field `type` (not `name`), per experiment 6.
/// Signature: entire type_alias node first line.
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

    // type_alias uses field "type" for the alias name, not "name"
    let name_node = match node.child_by_field_name("type") {
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

/// Handle a property_declaration node, extracting only const val.
///
/// Checks for `property_modifier` with text "const" inside `modifiers`.
/// Name is extracted from anonymous `variable_declaration` -> `identifier` child.
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

    // Check for const property_modifier
    if !is_const_property(node, source) {
        return;
    }

    // Extract name from variable_declaration -> identifier
    let var_decl = match first_child_by_kind(node, "variable_declaration") {
        Some(vd) => vd,
        None => return,
    };
    let name_node = match first_child_by_kind(var_decl, "identifier") {
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
        kind: DefKind::Const,
        lines: [start, end],
        signature,
        scope: own_scope,
    });
}

/// Check if a property_declaration has a const property_modifier.
fn is_const_property(node: Node, source: &str) -> bool {
    let modifiers = match first_child_by_kind(node, "modifiers") {
        Some(m) => m,
        None => return false,
    };
    let mut cursor = modifiers.walk();
    for child in modifiers.children(&mut cursor) {
        if child.kind() == "property_modifier" {
            if let Ok(text) = child.utf8_text(source.as_bytes()) {
                if text == "const" {
                    return true;
                }
            }
        }
    }
    false
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
            handle_class(node, source, mode, def_kind, kinds, results, scope);
        }
        "object_declaration" | "companion_object" => {
            handle_object(node, source, mode, kinds, results, scope);
        }
        "function_declaration" => {
            handle_function(node, source, mode, kinds, results, scope);
            // Do not recurse into function body
        }
        "type_alias" => {
            handle_typealias(node, source, mode, kinds, results, scope);
        }
        "property_declaration" => {
            handle_const(node, source, mode, kinds, results, scope);
            // Do not recurse -- property initializers do not contain nested definitions
        }
        // Skip: package and import are not definitions
        "package_header" | "import" => {}
        // Skip: secondary_constructor has no name field
        "secondary_constructor" => {}
        _ => {
            recurse_children(node, source, mode, kinds, results, scope);
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::extract_definitions;

    // === Bug verification: unnamed companion object functions are lost ===

    #[test]
    fn test_function_in_unnamed_companion_object() {
        let source = r#"
class MyClass {
    companion object {
        fun create(): MyClass = MyClass()
    }
}
"#;
        let defs = extract_definitions(&KotlinParser, "create", &[DefKind::Function], source);
        assert_eq!(
            defs.len(),
            1,
            "should find function inside unnamed companion object, but got {} results",
            defs.len()
        );
        assert_eq!(defs[0].kind, DefKind::Function);
        assert!(defs[0].signature.contains("fun create"));
    }

    #[test]
    fn test_const_in_unnamed_companion_object() {
        let source = r#"
class Config {
    companion object {
        const val TAG = "Config"
    }
}
"#;
        let defs = extract_definitions(&KotlinParser, "TAG", &[DefKind::Const], source);
        assert_eq!(
            defs.len(),
            1,
            "should find const inside unnamed companion object, but got {} results",
            defs.len()
        );
        assert_eq!(defs[0].kind, DefKind::Const);
        assert!(defs[0].signature.contains("TAG"));
    }

    #[test]
    fn test_function_in_named_companion_object() {
        let source = r#"
class MyClass {
    companion object Factory {
        fun create(): MyClass = MyClass()
    }
}
"#;
        let defs = extract_definitions(&KotlinParser, "create", &[DefKind::Function], source);
        assert_eq!(
            defs.len(),
            1,
            "should find function inside named companion object"
        );
        assert_eq!(defs[0].scope, "MyClass.Factory.create");
    }

    #[test]
    fn test_nested_class_in_unnamed_companion_object() {
        let source = r#"
class Outer {
    companion object {
        class Inner
    }
}
"#;
        let defs = extract_definitions(&KotlinParser, "Inner", &[DefKind::Class], source);
        assert_eq!(
            defs.len(),
            1,
            "should find nested class inside unnamed companion object"
        );
    }
}
