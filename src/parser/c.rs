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

    fn scope_separators(&self) -> &'static [&'static str] {
        &["::"]
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
        }
        "enum_specifier" => {
            handle_enum(node, source, mode, kinds, results);
        }
        "type_definition" => {
            handle_typedef(node, source, mode, kinds, results);
        }
        "declaration" if is_const_declaration(node, source) => {
            handle_const(node, source, mode, kinds, results);
        }
        "preproc_def" | "preproc_function_def" => {
            handle_macro(node, source, mode, kinds, results);
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
        scope: name_text,
    });
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

    // --- Meta tests ---

    #[test]
    fn test_language_returns_c() {
        let p = CParser;
        assert_eq!(p.language(), "c");
    }

    #[test]
    fn test_extensions_cover_c() {
        let p = CParser;
        assert!(p.extensions().contains(&".c"));
        assert_eq!(p.extensions().len(), 1);
    }

    #[test]
    fn test_supported_kinds_six() {
        let p = CParser;
        let kinds = p.supported_kinds();
        assert_eq!(kinds.len(), 6);
        assert!(kinds.contains(&DefKind::Function));
        assert!(kinds.contains(&DefKind::Struct));
        assert!(kinds.contains(&DefKind::Enum));
        assert!(kinds.contains(&DefKind::Type));
        assert!(kinds.contains(&DefKind::Const));
        assert!(kinds.contains(&DefKind::Macro));
    }

    // --- Edge case / handler tests ---

    #[test]
    fn test_struct_forward_declaration_skipped() {
        let results = extract_definitions(&CParser, "Node", &[DefKind::Struct], "struct Node;");
        assert!(results.is_empty());
    }

    #[test]
    fn test_typedef_struct_not_extracted_as_struct() {
        // typedef struct Point Point; -- the struct_specifier has no body, so skipped
        let results = extract_definitions(
            &CParser,
            "Point",
            &[DefKind::Struct],
            "typedef struct Point Point;",
        );
        assert!(results.is_empty());
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
    fn test_kind_filter_struct_not_func() {
        let src = "struct Foo { int x; };";
        let results = extract_definitions(&CParser, "Foo", &[DefKind::Function], src);
        assert!(results.is_empty());
    }

    #[test]
    fn test_kind_filter_enum_not_type() {
        let src = "enum Color { RED };";
        let results = extract_definitions(&CParser, "Color", &[DefKind::Type], src);
        assert!(results.is_empty());
    }

    #[test]
    fn test_kind_filter_type_not_const() {
        let src = "typedef int MyInt;";
        let results = extract_definitions(&CParser, "MyInt", &[DefKind::Const], src);
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
    fn test_macro_inside_struct() {
        let src = "struct Foo {\n#define INNER_MACRO 42\n    int x;\n};";
        let results = extract_definitions(&CParser, "INNER_MACRO", &[DefKind::Macro], src);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].kind, DefKind::Macro);
        assert_eq!(results[0].scope, "INNER_MACRO");
    }
}
