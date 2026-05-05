use crate::model::{DefContent, DefKind};
use crate::parser::{
    LanguageParser, MatchMode, build_scope, build_scope_from_node, first_child_by_kind,
    first_line_of_node, flatten_bytes, line_range, node_text, node_text_ref, normalize_signature,
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
            DefKind::Method,
            DefKind::Constructor,
            DefKind::Object,
            DefKind::Alias,
            DefKind::Const,
            DefKind::Property,
            DefKind::Package,
            DefKind::Variant,
        ]
    }

    impl_init_parser!(tree_sitter_kotlin_ng::LANGUAGE, "Kotlin");

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
    def_kind: DefKind,
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
        kind: def_kind,
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
    if !kinds.contains(&DefKind::Alias) {
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
        kind: DefKind::Alias,
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

/// Handle a non-const property_declaration in class body as Property kind.
fn handle_body_property(
    node: Node,
    source: &str,
    mode: &MatchMode,
    results: &mut Vec<DefContent>,
    scope: &str,
) {
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
        kind: DefKind::Property,
        lines: [start, end],
        signature,
        scope: own_scope,
    });
}

/// Extract primary_constructor from a class_declaration node.
///
/// primary_constructor is a child of class_declaration (not inside class_body).
/// Name is the class name (constructor has no separate name field).
/// Also extracts class_parameter nodes with val/var as Property.
fn extract_primary_constructor(
    class_node: Node,
    source: &str,
    mode: &MatchMode,
    kinds: &[DefKind],
    results: &mut Vec<DefContent>,
    scope: &str,
) {
    let want_ctor = kinds.contains(&DefKind::Constructor);
    let want_prop = kinds.contains(&DefKind::Property);

    if !want_ctor && !want_prop {
        return;
    }

    // Find primary_constructor child
    let mut cursor = class_node.walk();
    let pc_node = match class_node
        .children(&mut cursor)
        .find(|c| c.kind() == "primary_constructor")
    {
        Some(n) => n,
        None => return,
    };

    // Extract class_parameter with val/var as Property
    if want_prop {
        let class_own_scope = build_scope_from_node(class_node, source, scope, ".");
        extract_class_parameters(&pc_node, source, mode, results, &class_own_scope);
    }

    if !want_ctor {
        return;
    }

    // Name is the class name
    let name_node = match class_node.child_by_field_name("name") {
        Some(n) => n,
        None => return,
    };
    let class_name = node_text_ref(name_node, source);

    if !mode.matches_ident(class_name) {
        return;
    }

    let own_scope = build_scope(scope, ".", class_name);
    let start_row = pc_node.start_position().row + 1;
    let end_row = pc_node.end_position().row + 1;
    let sig = flatten_bytes(pc_node.start_byte(), pc_node.end_byte(), source)
        .unwrap_or_else(|| first_line_of_node(pc_node, source));
    let signature = normalize_signature(&sig);

    results.push(DefContent {
        kind: DefKind::Constructor,
        lines: [start_row as u32, end_row as u32],
        signature,
        scope: own_scope,
    });
}

/// Extract class_parameter nodes with val/var from a primary_constructor as Property.
fn extract_class_parameters(
    pc_node: &Node,
    source: &str,
    mode: &MatchMode,
    results: &mut Vec<DefContent>,
    scope: &str,
) {
    let mut cursor = pc_node.walk();
    for child in pc_node.children(&mut cursor) {
        if child.kind() != "class_parameters" {
            continue;
        }
        let mut cp_cursor = child.walk();
        for cp in child.children(&mut cp_cursor) {
            if cp.kind() != "class_parameter" {
                continue;
            }
            // Check if parameter has val/var (unnamed literal child)
            let has_val_var = cp
                .children(&mut cp.walk())
                .any(|c| !c.is_named() && (c.kind() == "val" || c.kind() == "var"));
            if !has_val_var {
                continue;
            }
            // Extract name: first identifier child
            let name_node = match cp
                .children(&mut cp.walk())
                .find(|c| c.kind() == "identifier")
            {
                Some(n) => n,
                None => continue,
            };
            let name_ref = node_text_ref(name_node, source);
            if !mode.matches_ident(name_ref) {
                continue;
            }
            let own_scope = build_scope(scope, ".", name_ref);
            let sig = first_line_of_node(cp, source);
            let signature = normalize_signature(&sig);
            let start_row = cp.start_position().row + 1;
            let [start, end] = line_range(start_row, cp);

            results.push(DefContent {
                kind: DefKind::Property,
                lines: [start, end],
                signature,
                scope: own_scope,
            });
        }
    }
}

/// Handle a secondary_constructor node.
///
/// secondary_constructor is inside class_body. Name is derived from scope
/// (the containing class name).
fn handle_secondary_constructor(
    node: Node,
    source: &str,
    mode: &MatchMode,
    kinds: &[DefKind],
    results: &mut Vec<DefContent>,
    scope: &str,
) {
    if !kinds.contains(&DefKind::Constructor) {
        return;
    }

    // Extract class name from scope (last segment after '.')
    let class_name = scope.rsplit('.').next().unwrap_or("");
    if class_name.is_empty() {
        return;
    }

    if !mode.matches_ident(class_name) {
        return;
    }

    // Scope is already the class scope (e.g. "Person"), set by handle_class's recurse_children.
    // Both constructors share the class scope -- no additional nesting.
    let start_row = node.start_position().row + 1;
    let end_row = node.end_position().row + 1;
    let sig = if let Some(body) = first_child_by_kind(node, "function_body") {
        flatten_bytes(node.start_byte(), body.start_byte(), source)
            .unwrap_or_else(|| first_line_of_node(node, source))
    } else if let Some(params) = first_child_by_kind(node, "class_parameters") {
        flatten_bytes(node.start_byte(), params.end_byte(), source)
            .unwrap_or_else(|| first_line_of_node(node, source))
    } else {
        first_line_of_node(node, source)
    };
    let signature = normalize_signature(&sig);

    results.push(DefContent {
        kind: DefKind::Constructor,
        lines: [start_row as u32, end_row as u32],
        signature,
        scope: scope.to_string(),
    });
}

/// Handle an enum_entry node (Kotlin enum variant).
/// enum_entry has a "name" field returning a simple_identifier.
/// Scope uses "." separator: e.g., "Color.RED".
fn handle_enum_entry(
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
    let sig = first_line_of_node(node, source);
    let signature = normalize_signature(&sig);
    let start_row = node.start_position().row + 1;
    let [start, end] = line_range(start_row, node);

    results.push(DefContent {
        kind: DefKind::Variant,
        lines: [start, end],
        signature,
        scope: own_scope,
    });
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
            // Extract primary_constructor if present (child of class_declaration)
            extract_primary_constructor(node, source, mode, kinds, results, scope);
        }
        "object_declaration" | "companion_object" => {
            handle_object(node, source, mode, kinds, results, scope);
        }
        "function_declaration" => {
            // Scope-based: if inside a class/object body (!scope.is_empty()), emit Method
            let def_kind = if scope.is_empty() {
                DefKind::Function
            } else {
                DefKind::Method
            };
            handle_function(node, source, mode, kinds, results, scope, def_kind);
            // Do not recurse into function body
        }
        "secondary_constructor" => {
            handle_secondary_constructor(node, source, mode, kinds, results, scope);
        }
        "type_alias" => {
            handle_typealias(node, source, mode, kinds, results, scope);
        }
        "property_declaration" => {
            if is_const_property(node, source) {
                handle_const(node, source, mode, kinds, results, scope);
            } else if kinds.contains(&DefKind::Property) {
                handle_body_property(node, source, mode, results, scope);
            }
            // Do not recurse -- property initializers do not contain nested definitions
        }
        "package_header" => {
            if kinds.contains(&DefKind::Package) {
                if let Some(name_node) = first_child_by_kind(node, "qualified_identifier") {
                    let name = node_text(name_node, source);
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
        }
        // Skip: import is not a definition
        "import" => {}
        "enum_entry" => {
            if kinds.contains(&DefKind::Variant) {
                handle_enum_entry(node, source, mode, results, scope);
            }
        }
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
        let defs = extract_definitions(&KotlinParser, "create", &[DefKind::Method], source);
        assert_eq!(
            defs.len(),
            1,
            "should find method inside unnamed companion object, but got {} results",
            defs.len()
        );
        assert_eq!(defs[0].kind, DefKind::Method);
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
        let defs = extract_definitions(&KotlinParser, "create", &[DefKind::Method], source);
        assert_eq!(
            defs.len(),
            1,
            "should find method inside named companion object"
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
