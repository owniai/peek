use crate::model::{DefContent, DefKind};
use crate::parser::{
    LanguageParser, MatchMode, build_scope, build_scope_from_node, extract_const_name,
    extract_function_name, extract_signature_to_body, extract_typedef_name, first_line_of_node,
    handle_macro, is_const_declaration, line_range, node_text, node_text_ref,
};
use tree_sitter::{Node, Parser};

pub struct CppParser;

impl LanguageParser for CppParser {
    fn language(&self) -> &'static str {
        "cpp"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &[".cpp", ".cxx", ".cc", ".hpp", ".hxx", ".hh", ".h"]
    }

    fn supported_kinds(&self) -> &'static [DefKind] {
        &[
            DefKind::Function,
            DefKind::Class,
            DefKind::Struct,
            DefKind::Enum,
            DefKind::Type,
            DefKind::Const,
            DefKind::Macro,
        ]
    }

    fn scope_separators(&self) -> &'static [&'static str] {
        &["::"]
    }

    impl_init_parser!(tree_sitter_cpp::LANGUAGE, "C++");

    impl_extract_with!(collect_definitions, scope: "");
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
        "namespace_definition" => {
            let ns_name = extract_namespace_name(node, source);
            let new_scope = build_scope(scope, "::", &ns_name);
            recurse_into_body(node, source, mode, kinds, results, &new_scope);
            return; // namespace_definition handles its own children
        }
        "class_specifier" => {
            handle_class_or_struct(node, source, mode, kinds, results, scope, DefKind::Class);
            // Recurse into class body for nested definitions
            let new_scope = build_scope_from_node(node, source, scope, "::");
            recurse_into_body(node, source, mode, kinds, results, &new_scope);
            return;
        }
        "struct_specifier" => {
            handle_class_or_struct(node, source, mode, kinds, results, scope, DefKind::Struct);
            // Recurse into struct body for nested definitions
            let new_scope = build_scope_from_node(node, source, scope, "::");
            recurse_into_body(node, source, mode, kinds, results, &new_scope);
            return;
        }
        "enum_specifier" => {
            handle_enum(node, source, mode, kinds, results, scope);
            // Enums don't contribute to scope
            return;
        }
        "function_definition" => {
            handle_function(node, source, mode, kinds, results, scope);
            // Do not recurse into function body
            return;
        }
        "template_declaration" => {
            // template_declaration wraps function_definition as anonymous child
            // Recurse into its children to find function_definition
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                collect_definitions(child, source, mode, kinds, results, scope);
            }
            return;
        }
        "type_definition" => {
            handle_typedef(node, source, mode, kinds, results, scope);
        }
        "alias_declaration" => {
            handle_alias(node, source, mode, kinds, results, scope);
        }
        "declaration" if is_const_declaration(node, source) => {
            handle_const(node, source, mode, kinds, results, scope);
        }
        "preproc_def" | "preproc_function_def" => {
            handle_macro(node, source, mode, kinds, results);
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
    scope: &str,
) {
    if !kinds.contains(&DefKind::Function) {
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

    let qualified = qualified_name_from_declarator(declarator, source);
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
        kind: DefKind::Function,
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

/// Handle class_specifier and struct_specifier nodes.
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

fn handle_typedef(
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
        kind: DefKind::Type,
        lines: [start, end],
        signature,
        scope: build_scope(scope, "::", &name_text),
    });
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
    if !kinds.contains(&DefKind::Type) {
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
        kind: DefKind::Type,
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

/// Extract namespace name from a namespace_definition node.
/// The name field contains a namespace_identifier node.
fn extract_namespace_name(node: Node, source: &str) -> String {
    match node.child_by_field_name("name") {
        Some(n) => node_text(n, source),
        None => String::new(),
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
) {
    if let Some(body) = node.child_by_field_name("body") {
        let mut cursor = body.walk();
        for child in body.children(&mut cursor) {
            collect_definitions(child, source, mode, kinds, results, scope);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::extract_definitions;

    // --- Meta tests ---

    #[test]
    fn test_language_returns_cpp() {
        let p = CppParser;
        assert_eq!(p.language(), "cpp");
    }

    #[test]
    fn test_extensions_cover_cpp() {
        let p = CppParser;
        assert!(p.extensions().contains(&".cpp"));
        assert!(p.extensions().contains(&".h"));
        assert!(p.extensions().contains(&".hpp"));
        assert_eq!(p.extensions().len(), 7);
    }

    #[test]
    fn test_supported_kinds_seven() {
        let p = CppParser;
        let kinds = p.supported_kinds();
        assert_eq!(kinds.len(), 7);
        assert!(kinds.contains(&DefKind::Function));
        assert!(kinds.contains(&DefKind::Class));
        assert!(kinds.contains(&DefKind::Struct));
        assert!(kinds.contains(&DefKind::Enum));
        assert!(kinds.contains(&DefKind::Type));
        assert!(kinds.contains(&DefKind::Const));
        assert!(kinds.contains(&DefKind::Macro));
    }

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
    fn test_enum_forward_declaration_skipped() {
        let results = extract_definitions(&CppParser, "Color", &[DefKind::Enum], "enum Color;");
        assert!(results.is_empty());
    }

    #[test]
    fn test_typedef_function_pointer_skipped() {
        let results = extract_definitions(
            &CppParser,
            "Comparator",
            &[DefKind::Type],
            "typedef int (*Comparator)(const void *, const void *);",
        );
        assert!(results.is_empty());
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
    fn test_kind_filter_class_not_func() {
        let src = "class Foo { public: void bar(); };";
        let results = extract_definitions(&CppParser, "Foo", &[DefKind::Function], src);
        assert!(results.is_empty());
    }

    #[test]
    fn test_out_of_class_method_found_by_short_name() {
        // Bug: extract_function_name returns "Engine::start" (full qualified name)
        // instead of "start", so searching for "start" yields no results.
        let src = "class Engine {\npublic:\n    void start();\n};\n\nvoid Engine::start() { }";
        let results = extract_definitions(&CppParser, "start", &[DefKind::Function], src);
        assert_eq!(
            results.len(),
            1,
            "BUG: out-of-class method 'void Engine::start()' not found when searching for 'start'. \
             extract_function_name returns 'Engine::start' instead of 'start'."
        );
    }

    #[test]
    fn test_out_of_class_method_scope_includes_class() {
        let src = "class Engine {\npublic:\n    void start();\n};\n\nvoid Engine::start() { }";
        let results = extract_definitions(&CppParser, "Engine::start", &[DefKind::Function], src);
        // Workaround: searching by full qualified name finds it
        assert_eq!(
            results.len(),
            1,
            "Searching by qualified name should find the method"
        );
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
    fn test_macro_not_matched_by_function() {
        let src = "#define FOO 42\nvoid FOO() {}";
        let results = extract_definitions(&CppParser, "FOO", &[DefKind::Macro], src);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].kind, DefKind::Macro);
        let results = extract_definitions(&CppParser, "FOO", &[DefKind::Function], src);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].kind, DefKind::Function);
    }
}
