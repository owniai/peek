use crate::model::{DefContent, DefKind};
use crate::parser::{
    LanguageParser, MatchMode, build_scope, build_scope_from_node, extract_const_name,
    extract_function_name, extract_signature_to_body, extract_static_name, extract_typedef_name,
    first_line_of_node, flatten_bytes, handle_macro, is_const_declaration, is_static_declaration,
    line_range, node_text, node_text_ref,
};
use tree_sitter::{Node, Parser};

pub struct CParser;

impl LanguageParser for CParser {
    fn language(&self) -> &'static str {
        "c"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &[".c"]
    }

    fn supported_kinds(&self) -> &'static [DefKind] {
        &[
            DefKind::Function,
            DefKind::Struct,
            DefKind::Union,
            DefKind::Enum,
            DefKind::Alias,
            DefKind::Const,
            DefKind::Macro,
            DefKind::Field,
            DefKind::Static,
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
            handle_function(node, source, mode, kinds, results);
            return;
        }
        "field_declaration" => {
            handle_field(node, source, mode, kinds, results, scope);
            return;
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
        "declaration" if is_static_declaration(node, source) => {
            handle_static(node, source, mode, kinds, results);
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
    if !mode.matches_ident(&name_text) {
        return;
    }

    let signature = extract_signature_to_body(node, source);
    let start_row = node.start_position().row + 1;
    let [start, end] = line_range(start_row, node);
    results.push(DefContent {
        kind: DefKind::Function,
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
    if !kinds.contains(&def_kind) {
        return;
    }

    // Only extract structs with a body (skip forward declarations)
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
        scope: name_text,
    });
}

fn handle_static(
    node: Node,
    source: &str,
    mode: &MatchMode,
    kinds: &[DefKind],
    results: &mut Vec<DefContent>,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::extract_definitions;

    // --- Edge case / handler tests ---

    #[test]
    fn test_struct_forward_declaration_skipped() {
        let results = extract_definitions(&CParser, "Node", &[DefKind::Struct], "struct Node;");
        assert!(results.is_empty());
    }

    #[test]
    fn test_typedef_struct_no_body_still_type() {
        // typedef struct Point Point; -- no body, just an alias, remains Type
        let results = extract_definitions(
            &CParser,
            "Point",
            &[DefKind::Struct],
            "typedef struct Point Point;",
        );
        assert!(
            results.is_empty(),
            "typedef alias without body should not be Struct"
        );
        let results = extract_definitions(
            &CParser,
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
            &CParser,
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
            &CParser,
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
            &CParser,
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
            &CParser,
            "DataT",
            &[DefKind::Union],
            "typedef union Data { int i; float f; } DataT;",
        );
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].kind, DefKind::Union);
        assert_eq!(results[0].scope, "DataT");
    }

    #[test]
    fn test_typedef_plain_type_still_type() {
        let results =
            extract_definitions(&CParser, "MyInt", &[DefKind::Alias], "typedef int MyInt;");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].kind, DefKind::Alias);
    }

    #[test]
    fn test_typedef_struct_with_body_not_type() {
        let results = extract_definitions(
            &CParser,
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
    fn test_enum_forward_declaration_skipped() {
        let results = extract_definitions(&CParser, "Color", &[DefKind::Enum], "enum Color;");
        assert!(results.is_empty());
    }

    #[test]
    fn test_typedef_function_pointer_skipped() {
        // Function pointer typedefs should not be extracted as Type
        let results = extract_definitions(
            &CParser,
            "Comparator",
            &[DefKind::Alias],
            "typedef int (*Comparator)(const void *, const void *);",
        );
        assert!(results.is_empty());
    }

    #[test]
    fn test_non_const_declaration_skipped() {
        let results = extract_definitions(&CParser, "x", &[DefKind::Const], "int x = 1;");
        assert!(results.is_empty());
    }

    #[test]
    fn test_const_return_function_prototype_not_const() {
        let results = extract_definitions(
            &CParser,
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
            &CParser,
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
    fn test_kind_filter_func_not_struct() {
        let src = "int foo() {}\nstruct Bar { int x; };";
        let results = extract_definitions(&CParser, "foo", &[DefKind::Struct], src);
        assert!(results.is_empty());
    }

    #[test]
    fn test_union_recognized_as_union() {
        let src = "union Value { int i; float f; };";
        let results = extract_definitions(&CParser, "Value", &[DefKind::Union], src);
        assert!(
            !results.is_empty(),
            "unions should be recognized as Union definitions"
        );
        assert_eq!(results[0].kind, DefKind::Union);
        assert_eq!(results[0].scope, "Value");
        assert!(results[0].signature.contains("union Value"));
    }

    #[test]
    fn test_macro_not_matched_by_function() {
        let src = "#define FOO 42\nint FOO() { return 0; }";
        let results = extract_definitions(&CParser, "FOO", &[DefKind::Macro], src);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].kind, DefKind::Macro);
        let results = extract_definitions(&CParser, "FOO", &[DefKind::Function], src);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].kind, DefKind::Function);
    }

    #[test]
    fn test_macro_kind_filter() {
        let src = "#define Config 42\nstruct Config { int x; };";
        let results = extract_definitions(&CParser, "Config", &[DefKind::Macro], src);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].kind, DefKind::Macro);
        let results = extract_definitions(&CParser, "Config", &[DefKind::Struct], src);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].kind, DefKind::Struct);
    }

    #[test]
    fn test_typedef_struct_no_double_extraction() {
        let results = extract_definitions(
            &CParser,
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
            &CParser,
            "INNER_MACRO",
            &[DefKind::Macro],
            "typedef struct { #define INNER_MACRO 42\n int x; } MyType;",
        );
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].kind, DefKind::Macro);
        assert_eq!(results[0].scope, "INNER_MACRO");
    }

    #[test]
    fn test_macro_inside_struct() {
        let src = "struct Foo {\n#define INNER_MACRO 42\n    int x;\n};";
        let results = extract_definitions(&CParser, "INNER_MACRO", &[DefKind::Macro], src);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].kind, DefKind::Macro);
        assert_eq!(results[0].scope, "INNER_MACRO");
    }

    #[test]
    fn test_union_body_recursion_finds_nested_macro() {
        let src = "union Data {\n#define UNION_MACRO 1\n    int i;\n    float f;\n};";
        let results = extract_definitions(&CParser, "UNION_MACRO", &[DefKind::Macro], src);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].kind, DefKind::Macro);
        assert_eq!(results[0].scope, "UNION_MACRO");
    }

    #[test]
    fn test_struct_enum_no_double_extraction_with_macro() {
        let src = "struct Foo {\n#define M 1\n    int x;\n};\nenum Bar { A, B };";
        let results = extract_definitions(
            &CParser,
            ".",
            &[DefKind::Struct, DefKind::Enum, DefKind::Macro],
            src,
        );
        assert_eq!(results.len(), 3);
        let kinds: Vec<_> = results.iter().map(|r| r.kind).collect();
        assert!(kinds.contains(&DefKind::Struct));
        assert!(kinds.contains(&DefKind::Enum));
        assert!(kinds.contains(&DefKind::Macro));
    }

    // --- Field extraction ---

    #[test]
    fn struct_field_extracted_as_field() {
        let src = "struct Point { double x; double y; };";
        let results = extract_definitions(&CParser, "x", &[DefKind::Field], src);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].kind, DefKind::Field);
        assert_eq!(results[0].scope, "Point::x");
    }

    #[test]
    fn struct_multiple_fields() {
        let src = "struct Config { int timeout; int retries; };";
        let results = extract_definitions(&CParser, "retries", &[DefKind::Field], src);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].kind, DefKind::Field);
        assert_eq!(results[0].scope, "Config::retries");
    }

    #[test]
    fn union_field_extracted_as_field() {
        let src = "union Value { int i; float f; };";
        let results = extract_definitions(&CParser, "i", &[DefKind::Field], src);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].kind, DefKind::Field);
        assert_eq!(results[0].scope, "Value::i");
    }

    #[test]
    fn field_kind_filter_excludes_struct() {
        let src = "struct Foo { int bar; };";
        let results = extract_definitions(&CParser, "bar", &[DefKind::Struct], src);
        assert!(
            results.is_empty(),
            "field should not match -k struct, got: {results:?}"
        );
    }

    #[test]
    fn pointer_field_extracted() {
        let src = "struct Node { char *name; int value; };";
        let results = extract_definitions(&CParser, "name", &[DefKind::Field], src);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].kind, DefKind::Field);
        assert_eq!(results[0].scope, "Node::name");
    }

    #[test]
    fn typedef_struct_field_extracted() {
        let src = "typedef struct { int x; int y; } PointT;";
        let results = extract_definitions(&CParser, "x", &[DefKind::Field], src);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].kind, DefKind::Field);
        assert_eq!(results[0].scope, "PointT::x");
    }

    // ============================================================
    // Static variable extraction tests
    // ============================================================

    #[test]
    fn static_var_extracted_as_static() {
        let src = "static int count = 0;";
        let results = extract_definitions(&CParser, "count", &[DefKind::Static], src);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].kind, DefKind::Static);
        assert_eq!(results[0].scope, "count");
    }

    #[test]
    fn static_var_without_initializer() {
        let src = "static int count;";
        let results = extract_definitions(&CParser, "count", &[DefKind::Static], src);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].kind, DefKind::Static);
    }

    #[test]
    fn static_const_is_const_not_static() {
        // `static const int VERSION = 2;` is Const, not Static
        let src = "static const int VERSION = 2;";
        let results = extract_definitions(&CParser, "VERSION", &[DefKind::Static], src);
        assert!(results.is_empty());
    }

    #[test]
    fn non_static_var_not_extracted_as_static() {
        let src = "int global = 42;";
        let results = extract_definitions(&CParser, "global", &[DefKind::Static], src);
        assert!(results.is_empty());
    }

    #[test]
    fn static_function_not_extracted_as_static() {
        // static functions are Function kind, not Static kind
        let src = "static void helper(void) {}";
        let results = extract_definitions(&CParser, "helper", &[DefKind::Static], src);
        assert!(results.is_empty());
    }

    #[test]
    fn static_pointer_var_extracted() {
        let src = "static char *name = \"test\";";
        let results = extract_definitions(&CParser, "name", &[DefKind::Static], src);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].kind, DefKind::Static);
        assert_eq!(results[0].scope, "name");
    }

    // ============================================================
    // Variant (enum member) extraction tests
    // ============================================================

    #[test]
    fn enum_variant_extracted_as_variant() {
        let src = "enum Color { RED, GREEN, BLUE };";
        let results = extract_definitions(&CParser, "RED", &[DefKind::Variant], src);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].kind, DefKind::Variant);
        assert_eq!(results[0].scope, "Color::RED");
    }

    #[test]
    fn enum_variant_with_value() {
        let src = "enum Status { OK = 0, ERR = -1 };";
        let results = extract_definitions(&CParser, "ERR", &[DefKind::Variant], src);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].kind, DefKind::Variant);
        assert_eq!(results[0].scope, "Status::ERR");
    }

    #[test]
    fn enum_multiple_variants() {
        let src = "enum Dir { NORTH, SOUTH, EAST, WEST };";
        let results = extract_definitions(&CParser, ".", &[DefKind::Variant], src);
        assert_eq!(results.len(), 4);
        let names: Vec<_> = results.iter().map(|r| r.scope.clone()).collect();
        assert!(names.contains(&"Dir::NORTH".to_string()));
        assert!(names.contains(&"Dir::SOUTH".to_string()));
        assert!(names.contains(&"Dir::EAST".to_string()));
        assert!(names.contains(&"Dir::WEST".to_string()));
    }

    #[test]
    fn variant_kind_filter_excludes_enum() {
        let src = "enum Color { RED };";
        let results = extract_definitions(&CParser, "RED", &[DefKind::Enum], src);
        assert!(
            results.is_empty(),
            "variant should not match -k enum, got: {results:?}"
        );
    }

    #[test]
    fn variant_not_extracted_when_kind_not_requested() {
        let src = "enum Color { RED };";
        let results = extract_definitions(&CParser, "RED", &[DefKind::Struct], src);
        assert!(results.is_empty());
    }

    #[test]
    fn enum_and_variants_together() {
        let src = "enum Color { RED, GREEN, BLUE };";
        let results = extract_definitions(&CParser, ".", &[DefKind::Enum, DefKind::Variant], src);
        assert_eq!(results.len(), 4);
        let kinds: Vec<_> = results.iter().map(|r| r.kind).collect();
        assert_eq!(kinds.iter().filter(|k| **k == DefKind::Enum).count(), 1);
        assert_eq!(kinds.iter().filter(|k| **k == DefKind::Variant).count(), 3);
    }

    #[test]
    fn typedef_enum_variant_scope() {
        let src = "typedef enum { ON, OFF } Switch;";
        let results = extract_definitions(&CParser, "ON", &[DefKind::Variant], src);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].kind, DefKind::Variant);
        assert_eq!(results[0].scope, "Switch::ON");
    }
}
