use crate::model::{DefContent, DefKind};
use crate::parser::{
    LanguageParser, MatchMode, extract_const_name, extract_function_name,
    extract_signature_to_body, extract_typedef_name, first_line_of_node, handle_macro,
    is_const_declaration, line_range, node_text_ref,
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
            DefKind::Enum,
            DefKind::Type,
            DefKind::Const,
            DefKind::Macro,
        ]
    }

    impl_init_parser!(tree_sitter_c::LANGUAGE, "C");

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
        "function_definition" => {
            handle_function(node, source, mode, kinds, results);
            return;
        }
        "struct_specifier" | "union_specifier" => {
            handle_struct(node, source, mode, kinds, results);
            if let Some(body) = node.child_by_field_name("body") {
                let mut cursor = body.walk();
                for child in body.children(&mut cursor) {
                    collect_definitions(child, source, mode, kinds, results);
                }
            }
            return;
        }
        "enum_specifier" => {
            handle_enum(node, source, mode, kinds, results);
            if let Some(body) = node.child_by_field_name("body") {
                let mut cursor = body.walk();
                for child in body.children(&mut cursor) {
                    collect_definitions(child, source, mode, kinds, results);
                }
            }
            return;
        }
        "type_definition" => {
            handle_typedef(node, source, mode, kinds, results);
            if let Some(type_node) = node.child_by_field_name("type") {
                if matches!(type_node.kind(), "struct_specifier" | "union_specifier") {
                    if let Some(body) = type_node.child_by_field_name("body") {
                        let mut cursor = body.walk();
                        for child in body.children(&mut cursor) {
                            collect_definitions(child, source, mode, kinds, results);
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
        "preproc_def" | "preproc_function_def" => {
            handle_macro(node, source, mode, kinds, results);
            return;
        }
        _ => {}
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_definitions(child, source, mode, kinds, results);
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

fn handle_struct(
    node: Node,
    source: &str,
    mode: &MatchMode,
    kinds: &[DefKind],
    results: &mut Vec<DefContent>,
) {
    if !kinds.contains(&DefKind::Struct) {
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
        kind: DefKind::Struct,
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
        if matches!(type_node.kind(), "struct_specifier" | "union_specifier")
            && type_node.child_by_field_name("body").is_some()
        {
            return DefKind::Struct;
        }
    }
    DefKind::Type
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
            &[DefKind::Type],
            "typedef struct Point Point;",
        );
        assert_eq!(results.len(), 1, "typedef alias should still be Type");
        assert_eq!(results[0].kind, DefKind::Type);
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
    fn test_typedef_union_with_body_is_struct() {
        let results = extract_definitions(
            &CParser,
            "UHandle",
            &[DefKind::Struct],
            "typedef union { int i; float f; } UHandle;",
        );
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].kind, DefKind::Struct);
        assert_eq!(results[0].scope, "UHandle");
    }

    #[test]
    fn test_typedef_named_union_with_body_is_struct() {
        let results = extract_definitions(
            &CParser,
            "DataT",
            &[DefKind::Struct],
            "typedef union Data { int i; float f; } DataT;",
        );
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].kind, DefKind::Struct);
        assert_eq!(results[0].scope, "DataT");
    }

    #[test]
    fn test_typedef_plain_type_still_type() {
        let results =
            extract_definitions(&CParser, "MyInt", &[DefKind::Type], "typedef int MyInt;");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].kind, DefKind::Type);
    }

    #[test]
    fn test_typedef_struct_with_body_not_type() {
        let results = extract_definitions(
            &CParser,
            "PointT",
            &[DefKind::Type],
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
            &[DefKind::Type],
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
    fn test_union_recognized_as_struct() {
        let src = "union Value { int i; float f; };";
        let results = extract_definitions(&CParser, "Value", &[DefKind::Struct], src);
        assert!(
            !results.is_empty(),
            "unions should be recognized as Struct definitions"
        );
        assert_eq!(results[0].kind, DefKind::Struct);
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
}
