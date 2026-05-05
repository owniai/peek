use crate::model::{DefContent, DefKind};
use crate::parser::{
    LanguageParser, MatchMode, build_scope, build_scope_from_node, extract_const_name,
    extract_function_name, extract_signature_to_body, extract_static_name, extract_typedef_name,
    first_line_of_node, handle_macro, is_const_declaration, is_static_declaration, line_range,
    node_text, node_text_ref,
};
use tree_sitter::{Node, Parser};

pub struct CppParser;

impl LanguageParser for CppParser {
    fn language(&self) -> &'static str {
        "cpp"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &[
            ".cpp", ".cxx", ".cc", ".hpp", ".hxx", ".hh", ".h", ".ixx", ".cppm", ".inl", ".tcc",
            ".tpp", ".ipp", ".inc", ".ino", ".txx",
        ]
    }

    fn supported_kinds(&self) -> &'static [DefKind] {
        &[
            DefKind::Function,
            DefKind::Method,
            DefKind::Class,
            DefKind::Struct,
            DefKind::Union,
            DefKind::Enum,
            DefKind::Alias,
            DefKind::Const,
            DefKind::Macro,
            DefKind::Field,
            DefKind::Static,
            DefKind::Namespace,
            DefKind::Variant,
            DefKind::Destructor,
        ]
    }

    impl_init_parser!(tree_sitter_cpp::LANGUAGE, "C++");

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
        collect_definitions(
            tree.root_node(),
            source,
            mode,
            kinds,
            &mut results,
            "",
            false,
        );
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
    in_type_body: bool,
) {
    match node.kind() {
        "namespace_definition" => {
            let ns_name = extract_namespace_name(node, source);
            let new_scope = build_scope(scope, "::", &ns_name);
            // Emit namespace definition (skip anonymous namespaces with empty name)
            if !ns_name.is_empty()
                && kinds.contains(&DefKind::Namespace)
                && mode.matches_ident(&new_scope)
            {
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
            recurse_into_body(node, source, mode, kinds, results, &new_scope, false);
            return; // namespace_definition handles its own children
        }
        "class_specifier" => {
            handle_class_or_struct(node, source, mode, kinds, results, scope, DefKind::Class);
            // Recurse into class body for nested definitions
            let new_scope = build_scope_from_node(node, source, scope, "::");
            recurse_into_body(node, source, mode, kinds, results, &new_scope, true);
            return;
        }
        "struct_specifier" => {
            handle_class_or_struct(node, source, mode, kinds, results, scope, DefKind::Struct);
            // Recurse into struct body for nested definitions
            let new_scope = build_scope_from_node(node, source, scope, "::");
            recurse_into_body(node, source, mode, kinds, results, &new_scope, true);
            return;
        }
        "union_specifier" => {
            handle_class_or_struct(node, source, mode, kinds, results, scope, DefKind::Union);
            // Recurse into union body for nested definitions
            let new_scope = build_scope_from_node(node, source, scope, "::");
            recurse_into_body(node, source, mode, kinds, results, &new_scope, true);
            return;
        }
        "enum_specifier" => {
            handle_enum(node, source, mode, kinds, results, scope);
            // Recurse into enum body with scope that includes the enum name,
            // so that enumerator children get proper scope like "Color::RED".
            let enum_scope = build_scope_from_node(node, source, scope, "::");
            recurse_into_body(
                node,
                source,
                mode,
                kinds,
                results,
                &enum_scope,
                in_type_body,
            );
            return;
        }
        "enumerator" => {
            handle_variant(node, source, mode, kinds, results, scope);
            return;
        }
        "function_definition" => {
            handle_function(node, source, mode, kinds, results, scope, in_type_body);
            // Do not recurse into function body
            return;
        }
        "template_declaration" => {
            // template_declaration wraps function_definition as anonymous child
            // Recurse into its children to find function_definition
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                collect_definitions(child, source, mode, kinds, results, scope, in_type_body);
            }
            return;
        }
        "type_definition" => {
            handle_typedef(node, source, mode, kinds, results, scope);
            if let Some(type_node) = node.child_by_field_name("type") {
                if matches!(type_node.kind(), "struct_specifier" | "union_specifier") {
                    recurse_into_body(type_node, source, mode, kinds, results, scope, true);
                }
            }
            return;
        }
        "alias_declaration" => {
            handle_alias(node, source, mode, kinds, results, scope);
            return;
        }
        "declaration" if is_const_declaration(node, source) => {
            handle_const(node, source, mode, kinds, results, scope);
            return;
        }
        "declaration" if is_static_declaration(node, source) => {
            handle_static(node, source, mode, kinds, results, scope);
            return;
        }
        "field_declaration" => {
            handle_field(node, source, mode, kinds, results, scope);
            // Do NOT return -- field_declaration may wrap nested type definitions
            // (e.g., `class Item { ... };` inside a class body is a field_declaration
            // wrapping a class_specifier). Fall through to recurse into children.
        }
        "preproc_def" | "preproc_function_def" => {
            handle_macro(node, source, mode, kinds, results);
            return;
        }
        "preproc_call" => {
            handle_preproc_call_macro(node, source, mode, kinds, results);
            return;
        }
        _ => {}
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_definitions(child, source, mode, kinds, results, scope, in_type_body);
    }
}

fn handle_function(
    node: Node,
    source: &str,
    mode: &MatchMode,
    kinds: &[DefKind],
    results: &mut Vec<DefContent>,
    scope: &str,
    in_type_body: bool,
) {
    let declarator = match node.child_by_field_name("declarator") {
        Some(d) => d,
        None => return,
    };

    // Check if this is a destructor (contains destructor_name in declarator chain)
    let destructor_name = find_destructor_name(declarator, source);
    if let Some(dtor_text) = &destructor_name {
        if !kinds.contains(&DefKind::Destructor) {
            return;
        }
        if !mode.matches_ident(dtor_text) {
            return;
        }
        let signature = extract_signature_to_body(node, source);
        let start_row = node.start_position().row + 1;
        let [start, end] = line_range(start_row, node);
        results.push(DefContent {
            kind: DefKind::Destructor,
            lines: [start, end],
            signature,
            scope: build_scope(scope, "::", dtor_text),
        });
        return;
    }

    let name_text = match extract_function_name(declarator, source) {
        Some(n) => n,
        None => return,
    };

    let qualified = qualified_name_from_declarator(declarator, source);
    // Method if: in class/struct body, or has qualified_identifier (out-of-class method)
    let def_kind = if in_type_body || qualified.is_some() {
        DefKind::Method
    } else {
        DefKind::Function
    };

    if !kinds.contains(&def_kind) {
        return;
    }

    let matches = match &qualified {
        Some(qname) => mode.matches_ident(&name_text) || mode.matches_ident(qname),
        None => mode.matches_ident(&name_text),
    };
    if !matches {
        return;
    }

    let scope_name = qualified.as_deref().unwrap_or(&name_text);
    let signature = extract_signature_to_body(node, source);
    let start_row = node.start_position().row + 1;
    let [start, end] = line_range(start_row, node);
    results.push(DefContent {
        kind: def_kind,
        lines: [start, end],
        signature,
        scope: build_scope(scope, "::", scope_name),
    });
}

/// Extract the full qualified name (e.g., "Engine::start") from a declarator
/// chain if the innermost declarator is a `qualified_identifier`.
fn qualified_name_from_declarator(declarator: Node, source: &str) -> Option<String> {
    let mut current = declarator;
    loop {
        match current.kind() {
            "pointer_declarator" => {
                current = current.child_by_field_name("declarator")?;
            }
            "parenthesized_declarator" => {
                let mut cursor = current.walk();
                let child = current.children(&mut cursor).find(|c| {
                    matches!(
                        c.kind(),
                        "pointer_declarator"
                            | "parenthesized_declarator"
                            | "function_declarator"
                            | "identifier"
                    )
                })?;
                current = child;
            }
            "function_declarator" => {
                let inner = current.child_by_field_name("declarator")?;
                if inner.kind() == "qualified_identifier" {
                    return inner
                        .utf8_text(source.as_bytes())
                        .ok()
                        .map(|s| s.to_string());
                }
                return None;
            }
            _ => return None,
        }
    }
}

/// Handle class_specifier, struct_specifier, and union_specifier nodes.
/// Both share the same AST structure: name field + optional body field.
fn handle_class_or_struct(
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

    // Only extract classes/structs with a body (skip forward declarations)
    if node.child_by_field_name("body").is_none() {
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

    let signature = extract_signature_to_body(node, source);
    let start_row = node.start_position().row + 1;
    let [start, end] = line_range(start_row, node);
    results.push(DefContent {
        kind: def_kind,
        lines: [start, end],
        signature,
        scope: build_scope(scope, "::", &name),
    });
}

fn handle_enum(
    node: Node,
    source: &str,
    mode: &MatchMode,
    kinds: &[DefKind],
    results: &mut Vec<DefContent>,
    scope: &str,
) {
    if !kinds.contains(&DefKind::Enum) {
        return;
    }

    // Only extract enums with a body (skip forward declarations)
    if node.child_by_field_name("body").is_none() {
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

    let signature = first_line_of_node(node, source);
    let start_row = node.start_position().row + 1;
    let [start, end] = line_range(start_row, node);
    results.push(DefContent {
        kind: DefKind::Enum,
        lines: [start, end],
        signature,
        scope: build_scope(scope, "::", &name),
    });
}

/// Handle an enumerator node (C/C++ enum variant).
/// enumerator has a "name" field; fallback to first identifier child.
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

    let signature = first_line_of_node(node, source);
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

fn handle_typedef(
    node: Node,
    source: &str,
    mode: &MatchMode,
    kinds: &[DefKind],
    results: &mut Vec<DefContent>,
    scope: &str,
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
        scope: build_scope(scope, "::", &name_text),
    });
}

fn resolve_typedef_kind(node: Node) -> DefKind {
    if let Some(type_node) = node.child_by_field_name("type") {
        if type_node.child_by_field_name("body").is_some() {
            return match type_node.kind() {
                "struct_specifier" => DefKind::Struct,
                "union_specifier" => DefKind::Union,
                _ => DefKind::Alias,
            };
        }
    }
    DefKind::Alias
}

/// Handle alias_declaration (C++ using = type alias).
/// AST: alias_declaration -> name: type_identifier, type: type_descriptor
fn handle_alias(
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
        kind: DefKind::Alias,
        lines: [start, end],
        signature,
        scope: build_scope(scope, "::", &name),
    });
}

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
        kind: DefKind::Const,
        lines: [start, end],
        signature,
        scope: build_scope(scope, "::", &name_text),
    });
}

fn handle_static(
    node: Node,
    source: &str,
    mode: &MatchMode,
    kinds: &[DefKind],
    results: &mut Vec<DefContent>,
    scope: &str,
) {
    if !kinds.contains(&DefKind::Static) {
        return;
    }

    let declarator = match node.child_by_field_name("declarator") {
        Some(d) => d,
        None => return,
    };
    let name_text = match extract_static_name(declarator, source) {
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
        kind: DefKind::Static,
        lines: [start, end],
        signature,
        scope: build_scope(scope, "::", &name_text),
    });
}

/// Handle a field_declaration node inside a struct/class/union body.
///
/// C++ field_declaration has a `declarator` field pointing to `field_identifier`
/// (possibly wrapped in `pointer_declarator`). Static fields (with
/// `storage_class_specifier` child) are also Field kind, not Static kind --
/// per requirements rule 5, static modifier on class fields does NOT change
/// their kind.
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

    let signature = first_line_of_node(node, source);
    let start_row = node.start_position().row + 1;
    let [start, end] = line_range(start_row, node);
    results.push(DefContent {
        kind: DefKind::Field,
        lines: [start, end],
        signature,
        scope: build_scope(scope, "::", &name_text),
    });
}

/// Extract field name from a declarator field of a field_declaration node.
///
/// Handles pointer_declarator wrapping (e.g., `char *name` where declarator is
/// `pointer_declarator` wrapping `field_identifier`).
///
/// Returns None for function declarations (where declarator is a function_declarator),
/// as these are method declarations, not data fields.
fn extract_field_name(declarator: Node, source: &str) -> Option<String> {
    let mut current = declarator;
    loop {
        match current.kind() {
            "pointer_declarator" => {
                current = current.child_by_field_name("declarator")?;
            }
            "function_declarator" => return None,
            _ => break,
        }
    }
    let text = current.utf8_text(source.as_bytes()).ok()?;
    Some(text.to_string())
}

/// Extract namespace name from a namespace_definition node.
/// The name field contains a namespace_identifier node.
fn extract_namespace_name(node: Node, source: &str) -> String {
    match node.child_by_field_name("name") {
        Some(n) => node_text(n, source),
        None => String::new(),
    }
}

/// Handle `preproc_call` nodes that tree-sitter misparses as `#define` inside
/// enum bodies. In normal contexts `#define` becomes `preproc_def`, but inside
/// `enumerator_list` the C++ grammar produces `preproc_call` instead.
fn handle_preproc_call_macro(
    node: Node,
    source: &str,
    mode: &MatchMode,
    kinds: &[DefKind],
    results: &mut Vec<DefContent>,
) {
    if !kinds.contains(&DefKind::Macro) {
        return;
    }

    let mut cursor = node.walk();
    let is_define = node.children(&mut cursor).any(|child| {
        child.kind() == "preproc_directive" && node_text_ref(child, source) == "#define"
    });
    if !is_define {
        return;
    }

    let arg_node = match node
        .children(&mut cursor)
        .find(|c| c.kind() == "preproc_arg")
    {
        Some(n) => n,
        None => return,
    };
    let arg_text = node_text_ref(arg_node, source);
    let name = match arg_text.split_whitespace().next() {
        Some(n) => n,
        None => return,
    };

    if !mode.matches_ident(name) {
        return;
    }

    let signature = first_line_of_node(node, source);
    let start_row = node.start_position().row + 1;
    let [start, end] = line_range(start_row, node);
    results.push(DefContent {
        kind: DefKind::Macro,
        lines: [start, end],
        signature,
        scope: name.to_string(),
    });
}

/// Search the declarator chain for a `destructor_name` node.
///
/// In C++, destructors like `~Foo()` or `Foo::~Foo()` produce a
/// `destructor_name` node inside the `function_declarator`. This function
/// traverses pointer/parenthesized wrappers to find it.
///
/// Returns the destructor name text (e.g., "~Foo") if found.
fn find_destructor_name(declarator: Node, source: &str) -> Option<String> {
    let mut current = declarator;
    loop {
        match current.kind() {
            "pointer_declarator" => {
                current = current.child_by_field_name("declarator")?;
            }
            "parenthesized_declarator" => {
                let mut cursor = current.walk();
                let child = current.children(&mut cursor).find(|c| {
                    matches!(
                        c.kind(),
                        "pointer_declarator" | "parenthesized_declarator" | "function_declarator"
                    )
                })?;
                current = child;
            }
            "function_declarator" => {
                let inner = current.child_by_field_name("declarator")?;
                if inner.kind() == "destructor_name" {
                    return inner
                        .utf8_text(source.as_bytes())
                        .ok()
                        .map(|s| s.to_string());
                }
                // Also check for qualified_identifier wrapping destructor_name
                // (e.g., Foo::~Foo)
                if inner.kind() == "qualified_identifier" {
                    let mut cursor = inner.walk();
                    if let Some(dtor) = inner
                        .children(&mut cursor)
                        .find(|c| c.kind() == "destructor_name")
                    {
                        return dtor
                            .utf8_text(source.as_bytes())
                            .ok()
                            .map(|s| s.to_string());
                    }
                }
                return None;
            }
            _ => return None,
        }
    }
}

/// Recurse into a node's body field children.
fn recurse_into_body(
    node: Node,
    source: &str,
    mode: &MatchMode,
    kinds: &[DefKind],
    results: &mut Vec<DefContent>,
    scope: &str,
    in_type_body: bool,
) {
    if let Some(body) = node.child_by_field_name("body") {
        let mut cursor = body.walk();
        for child in body.children(&mut cursor) {
            collect_definitions(child, source, mode, kinds, results, scope, in_type_body);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::extract_definitions;

    // --- Edge case / handler tests ---

    #[test]
    fn test_class_forward_declaration_skipped() {
        let results = extract_definitions(&CppParser, "Node", &[DefKind::Class], "class Node;");
        assert!(results.is_empty());
    }

    #[test]
    fn test_struct_extracted_as_class_kind() {
        // C++ struct with class keyword regex pattern; struct_specifier is handled
        // by Class regex pattern but extracted with Struct kind if body present
        let results = extract_definitions(
            &CppParser,
            "Config",
            &[DefKind::Struct],
            "struct Config { int x; };",
        );
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].kind, DefKind::Struct);
    }

    #[test]
    fn test_struct_forward_declaration_skipped() {
        let results = extract_definitions(&CppParser, "Node", &[DefKind::Struct], "struct Node;");
        assert!(results.is_empty());
    }

    #[test]
    fn test_union_extracted_as_union_kind() {
        let results = extract_definitions(
            &CppParser,
            "Packet",
            &[DefKind::Union],
            "union Packet { struct { int x; int y; } coords; unsigned long raw; };",
        );
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].kind, DefKind::Union);
        assert_eq!(results[0].scope, "Packet");
    }

    #[test]
    fn test_union_forward_declaration_skipped() {
        let results = extract_definitions(&CppParser, "Data", &[DefKind::Union], "union Data;");
        assert!(results.is_empty());
    }

    #[test]
    fn test_enum_forward_declaration_skipped() {
        let results = extract_definitions(&CppParser, "Color", &[DefKind::Enum], "enum Color;");
        assert!(results.is_empty());
    }

    #[test]
    fn test_typedef_function_pointer_skipped() {
        let results = extract_definitions(
            &CppParser,
            "Comparator",
            &[DefKind::Alias],
            "typedef int (*Comparator)(const void *, const void *);",
        );
        assert!(results.is_empty());
    }

    #[test]
    fn test_typedef_struct_no_body_still_type() {
        let results = extract_definitions(
            &CppParser,
            "Point",
            &[DefKind::Struct],
            "typedef struct Point Point;",
        );
        assert!(
            results.is_empty(),
            "typedef alias without body should not be Struct"
        );
        let results = extract_definitions(
            &CppParser,
            "Point",
            &[DefKind::Alias],
            "typedef struct Point Point;",
        );
        assert_eq!(results.len(), 1, "typedef alias should still be Type");
        assert_eq!(results[0].kind, DefKind::Alias);
    }

    #[test]
    fn test_typedef_struct_with_body_is_struct() {
        let results = extract_definitions(
            &CppParser,
            "PointT",
            &[DefKind::Struct],
            "typedef struct Point { int x; int y; } PointT;",
        );
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].kind, DefKind::Struct);
        assert_eq!(results[0].scope, "PointT");
    }

    #[test]
    fn test_typedef_anon_struct_with_body_is_struct() {
        let results = extract_definitions(
            &CppParser,
            "AnonT",
            &[DefKind::Struct],
            "typedef struct { int x; } AnonT;",
        );
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].kind, DefKind::Struct);
        assert_eq!(results[0].scope, "AnonT");
    }

    #[test]
    fn test_typedef_union_with_body_is_union() {
        let results = extract_definitions(
            &CppParser,
            "UHandle",
            &[DefKind::Union],
            "typedef union { int i; float f; } UHandle;",
        );
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].kind, DefKind::Union);
        assert_eq!(results[0].scope, "UHandle");
    }

    #[test]
    fn test_typedef_named_union_with_body_is_union() {
        let results = extract_definitions(
            &CppParser,
            "DataT",
            &[DefKind::Union],
            "typedef union Data { int i; float f; } DataT;",
        );
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].kind, DefKind::Union);
        assert_eq!(results[0].scope, "DataT");
    }

    #[test]
    fn test_typedef_struct_with_body_not_type() {
        let results = extract_definitions(
            &CppParser,
            "PointT",
            &[DefKind::Alias],
            "typedef struct Point { int x; int y; } PointT;",
        );
        assert!(
            results.is_empty(),
            "typedef struct with body should be Struct, not Type"
        );
    }

    #[test]
    fn test_non_const_declaration_skipped() {
        let results = extract_definitions(&CppParser, "x", &[DefKind::Const], "int x = 1;");
        assert!(results.is_empty());
    }

    #[test]
    fn test_const_return_function_prototype_not_const() {
        // Bug: is_const_declaration treats const-return function prototypes as const variables.
        // extract_const_name extracts "compute()" (with parens), so searching "compute()" as
        // Const would match — demonstrating the misidentification.
        let results = extract_definitions(
            &CppParser,
            "compute()",
            &[DefKind::Const],
            "const int compute();",
        );
        assert!(
            results.is_empty(),
            "function prototype should not be a Const: {:?}",
            results
        );
    }

    #[test]
    fn test_const_return_pointer_function_prototype_not_const() {
        let results = extract_definitions(
            &CppParser,
            "get_buf()",
            &[DefKind::Const],
            "const int *get_buf();",
        );
        assert!(
            results.is_empty(),
            "pointer-return function prototype should not be a Const: {:?}",
            results
        );
    }

    #[test]
    fn test_kind_filter_func_not_class() {
        let src = "int foo() {}\nclass Bar { public: void bar(); };";
        let results = extract_definitions(&CppParser, "foo", &[DefKind::Class], src);
        assert!(results.is_empty());
    }

    #[test]
    fn test_out_of_class_method_found_by_short_name() {
        let src = "class Engine {\npublic:\n    void start();\n};\n\nvoid Engine::start() { }";
        let results = extract_definitions(&CppParser, "start", &[DefKind::Method], src);
        assert_eq!(
            results.len(),
            1,
            "out-of-class method 'void Engine::start()' should be found as Method when searching for 'start'"
        );
        assert_eq!(results[0].kind, DefKind::Method);
    }

    #[test]
    fn test_out_of_class_method_scope_includes_class() {
        let src = "class Engine {\npublic:\n    void start();\n};\n\nvoid Engine::start() { }";
        let results = extract_definitions(&CppParser, "Engine::start", &[DefKind::Method], src);
        assert_eq!(
            results.len(),
            1,
            "Searching by qualified name should find the method"
        );
        assert_eq!(results[0].kind, DefKind::Method);
    }

    #[test]
    fn test_macro_in_namespace() {
        let src = "namespace MyNS {\n#define NS_MACRO 100\n}";
        let results = extract_definitions(&CppParser, "NS_MACRO", &[DefKind::Macro], src);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].kind, DefKind::Macro);
        assert_eq!(results[0].scope, "NS_MACRO");
    }

    #[test]
    fn test_macro_inside_enum() {
        let src = "enum Color {\n#define INNER_MACRO 42\n    RED,\n    GREEN,\n    BLUE\n};";
        let results = extract_definitions(&CppParser, "INNER_MACRO", &[DefKind::Macro], src);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].kind, DefKind::Macro);
        assert_eq!(results[0].scope, "INNER_MACRO");
    }

    #[test]
    fn test_typedef_struct_no_double_extraction() {
        let results = extract_definitions(
            &CppParser,
            ".",
            &[DefKind::Struct],
            "typedef struct Point { int x; int y; } PointT;",
        );
        assert_eq!(
            results.len(),
            1,
            "should extract only PointT, not also Point"
        );
        assert_eq!(results[0].scope, "PointT");
    }

    #[test]
    fn test_macro_inside_typedef_struct() {
        let results = extract_definitions(
            &CppParser,
            "INNER_MACRO",
            &[DefKind::Macro],
            "typedef struct { #define INNER_MACRO 42\n int x; } MyType;",
        );
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].kind, DefKind::Macro);
        assert_eq!(results[0].scope, "INNER_MACRO");
    }

    #[test]
    fn test_macro_not_matched_by_function() {
        let src = "#define FOO 42\nvoid FOO() {}";
        let results = extract_definitions(&CppParser, "FOO", &[DefKind::Macro], src);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].kind, DefKind::Macro);
        let results = extract_definitions(&CppParser, "FOO", &[DefKind::Function], src);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].kind, DefKind::Function);
    }

    // ============================================================
    // Field extraction tests
    // ============================================================

    #[test]
    fn struct_field_extracted_as_field() {
        let src = "struct Point { double x; double y; };";
        let results = extract_definitions(&CppParser, "x", &[DefKind::Field], src);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].kind, DefKind::Field);
        assert_eq!(results[0].scope, "Point::x");
    }

    #[test]
    fn struct_multiple_fields() {
        let src = "struct Config { int timeout; int retries; };";
        let results = extract_definitions(&CppParser, "timeout", &[DefKind::Field], src);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].scope, "Config::timeout");
    }

    #[test]
    fn class_field_extracted_as_field() {
        let src = "class Engine { public: int power; };";
        let results = extract_definitions(&CppParser, "power", &[DefKind::Field], src);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].kind, DefKind::Field);
        assert_eq!(results[0].scope, "Engine::power");
    }

    #[test]
    fn field_kind_filter_excludes_struct() {
        let src = "struct Point { double x; };";
        let results = extract_definitions(&CppParser, "x", &[DefKind::Struct], src);
        assert!(results.is_empty());
    }

    #[test]
    fn static_field_still_field_not_static() {
        // Static fields in struct/class are Field kind, not Static kind
        let src = "struct Config { static int count; };";
        let results = extract_definitions(&CppParser, "count", &[DefKind::Field], src);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].kind, DefKind::Field);
    }

    // ============================================================
    // Static variable extraction tests
    // ============================================================

    #[test]
    fn static_var_extracted_as_static() {
        let src = "static int buffer_size = 4096;";
        let results = extract_definitions(&CppParser, "buffer_size", &[DefKind::Static], src);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].kind, DefKind::Static);
        assert_eq!(results[0].scope, "buffer_size");
    }

    #[test]
    fn static_var_in_namespace() {
        let src = "namespace Core { static int count = 0; }";
        let results = extract_definitions(&CppParser, "count", &[DefKind::Static], src);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].kind, DefKind::Static);
        assert_eq!(results[0].scope, "Core::count");
    }

    #[test]
    fn static_const_is_const_not_static() {
        // `static const int X = 1;` is Const kind, not Static kind
        let src = "static const int VERSION = 2;";
        let results = extract_definitions(&CppParser, "VERSION", &[DefKind::Static], src);
        assert!(results.is_empty());
    }

    #[test]
    fn non_static_var_not_extracted_as_static() {
        let src = "int global = 42;";
        let results = extract_definitions(&CppParser, "global", &[DefKind::Static], src);
        assert!(results.is_empty());
    }

    // ============================================================
    // Variant (enumerator) extraction tests
    // ============================================================

    #[test]
    fn enum_variant_extracted_as_variant() {
        let src = "enum Color { RED, GREEN, BLUE };";
        let results = extract_definitions(&CppParser, "RED", &[DefKind::Variant], src);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].kind, DefKind::Variant);
        assert_eq!(results[0].scope, "Color::RED");
    }

    #[test]
    fn enum_variant_with_value() {
        let src = "enum Status { OK = 0, ERROR = 1 };";
        let results = extract_definitions(&CppParser, "ERROR", &[DefKind::Variant], src);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].kind, DefKind::Variant);
        assert_eq!(results[0].scope, "Status::ERROR");
    }

    #[test]
    fn enum_variant_in_namespace() {
        let src = "namespace App { enum Color { RED, GREEN }; }";
        let results = extract_definitions(&CppParser, "GREEN", &[DefKind::Variant], src);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].kind, DefKind::Variant);
        assert_eq!(results[0].scope, "App::Color::GREEN");
    }

    #[test]
    fn enum_variant_kind_filter_excludes_method() {
        let src = "enum Color { RED, GREEN };";
        let results = extract_definitions(&CppParser, "RED", &[DefKind::Method], src);
        assert!(results.is_empty());
    }

    #[test]
    fn enum_class_variant() {
        let src = "enum class Direction { Up, Down, Left, Right };";
        let results = extract_definitions(&CppParser, "Up", &[DefKind::Variant], src);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].kind, DefKind::Variant);
        assert_eq!(results[0].scope, "Direction::Up");
    }

    #[test]
    fn enum_all_variants_dot_match() {
        let src = "enum Color { RED, GREEN, BLUE };";
        let results = extract_definitions(&CppParser, ".", &[DefKind::Variant], src);
        assert_eq!(results.len(), 3);
        assert_eq!(results[0].scope, "Color::RED");
        assert_eq!(results[1].scope, "Color::GREEN");
        assert_eq!(results[2].scope, "Color::BLUE");
    }

    // ============================================================
    // Destructor extraction tests
    // ============================================================

    #[test]
    fn destructor_in_class_body() {
        let src = "class Foo { public: ~Foo() { } };";
        let results = extract_definitions(&CppParser, "~Foo", &[DefKind::Destructor], src);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].kind, DefKind::Destructor);
        assert_eq!(results[0].scope, "Foo::~Foo");
    }

    #[test]
    fn destructor_out_of_class_definition() {
        let src = "class Foo { public: ~Foo(); };\nFoo::~Foo() { }";
        let results = extract_definitions(&CppParser, "~Foo", &[DefKind::Destructor], src);
        // The in-class declaration `~Foo();` is a declaration (not function_definition),
        // so only the out-of-class definition `Foo::~Foo() {}` is extracted.
        assert_eq!(
            results.len(),
            1,
            "should find out-of-class destructor definition"
        );
        assert_eq!(results[0].kind, DefKind::Destructor);
    }

    #[test]
    fn destructor_not_matched_as_method() {
        let src = "class Foo { public: ~Foo() { } };";
        let results = extract_definitions(&CppParser, "~Foo", &[DefKind::Method], src);
        assert!(
            results.is_empty(),
            "destructor should not be matched as Method"
        );
    }

    #[test]
    fn destructor_in_namespace() {
        let src = "namespace NS { class Bar { public: ~Bar() { } }; }";
        let results = extract_definitions(&CppParser, "~Bar", &[DefKind::Destructor], src);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].kind, DefKind::Destructor);
        assert_eq!(results[0].scope, "NS::Bar::~Bar");
    }
}
