use crate::model::{DefContent, DefKind};
use crate::parser::{
    LanguageParser, MatchMode, build_scope, first_line_of_node, flatten_bytes, line_range,
    node_text, node_text_ref,
};
use tree_sitter::{Node, Parser};

pub struct RustParser;

impl LanguageParser for RustParser {
    fn language(&self) -> &'static str {
        "rs"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &[".rs"]
    }

    fn supported_kinds(&self) -> &'static [DefKind] {
        &[
            DefKind::Function,
            DefKind::Struct,
            DefKind::Enum,
            DefKind::Type,
            DefKind::Trait,
            DefKind::Const,
            DefKind::Macro,
            DefKind::Module,
        ]
    }

    impl_init_parser!(tree_sitter_rust::LANGUAGE, "Rust");

    impl_extract_with!(collect_definitions, scope: "");
}

fn kind_for_node(node: Node) -> Option<DefKind> {
    match node.kind() {
        "function_item" => Some(DefKind::Function),
        "function_signature_item" => Some(DefKind::Function),
        "struct_item" => Some(DefKind::Struct),
        "enum_item" => Some(DefKind::Enum),
        "type_item" => Some(DefKind::Type),
        "trait_item" => Some(DefKind::Trait),
        "const_item" => Some(DefKind::Const),
        "macro_definition" => Some(DefKind::Macro),
        "mod_item" => Some(DefKind::Module),
        _ => None,
    }
}

fn extract_name(node: Node, source: &str) -> Option<String> {
    node.child_by_field_name("name")
        .map(|n| node_text(n, source))
}

fn extract_name_ref<'a>(node: Node, source: &'a str) -> Option<&'a str> {
    node.child_by_field_name("name")
        .map(|n| node_text_ref(n, source))
}

fn compute_impl_scope(impl_node: Node, source: &str, scope: &str) -> String {
    let type_node = match impl_node.child_by_field_name("type") {
        Some(t) => t,
        None => return scope.to_string(),
    };
    match find_type_identifier(type_node, source) {
        Some(name) => build_scope(scope, "::", &name),
        None => scope.to_string(),
    }
}

fn find_type_identifier(node: Node, source: &str) -> Option<String> {
    if node.kind() == "type_identifier" {
        return Some(node_text(node, source));
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if let Some(result) = find_type_identifier(child, source) {
            return Some(result);
        }
    }
    None
}

fn collect_definitions<'a>(
    node: Node<'a>,
    source: &str,
    mode: &MatchMode,
    kinds: &[DefKind],
    results: &mut Vec<DefContent>,
    scope: &str,
) {
    let mut attr_buffer: Vec<Node<'a>> = Vec::new();
    let mut cursor = node.walk();

    for child in node.children(&mut cursor) {
        match child.kind() {
            "attribute_item" => {
                attr_buffer.push(child);
            }
            "line_comment" | "block_comment" => {}
            // tree-sitter-rust can emit ERROR nodes (e.g. "pub" before "macro_rules!"),
            // skip them to prevent clearing attr_buffer and losing preceding attributes
            "ERROR" => {}
            "function_item" => {
                try_add_definition(child, source, mode, kinds, results, scope, &attr_buffer);
                attr_buffer.clear();
                // Don't recurse into function body — function-scoped definitions are not extracted
            }
            "impl_item" => {
                attr_buffer.clear();
                let new_scope = compute_impl_scope(child, source, scope);
                let mut inner_cursor = child.walk();
                for grandchild in child.children(&mut inner_cursor) {
                    collect_definitions(grandchild, source, mode, kinds, results, &new_scope);
                }
            }
            _ => {
                try_add_definition(child, source, mode, kinds, results, scope, &attr_buffer);
                attr_buffer.clear();
                let child_scope = match child.kind() {
                    "trait_item" | "mod_item" => {
                        let name = extract_name(child, source).unwrap_or_default();
                        build_scope(scope, "::", &name)
                    }
                    _ => scope.to_string(),
                };
                collect_definitions(child, source, mode, kinds, results, &child_scope);
            }
        }
    }
}

fn try_add_definition<'a>(
    node: Node<'a>,
    source: &str,
    mode: &MatchMode,
    kinds: &[DefKind],
    results: &mut Vec<DefContent>,
    scope: &str,
    attrs: &[Node<'a>],
) {
    let def_kind = match kind_for_node(node) {
        Some(k) => k,
        None => return,
    };

    if !kinds.contains(&def_kind) {
        return;
    }

    if let Some(name_ref) = extract_name_ref(node, source) {
        if mode.matches_ident(name_ref) {
            let name = name_ref.to_string();
            let scope = build_scope(scope, "::", &name);
            let start_byte = attrs
                .first()
                .map(|a| a.start_byte())
                .unwrap_or_else(|| node.start_byte());
            let end_byte = {
                let mut cursor = node.walk();
                let mut end = node.end_byte();
                for child in node.children(&mut cursor) {
                    match child.kind() {
                        "block"
                        | "field_declaration_list"
                        | "enum_variant_list"
                        | "declaration_list" => {
                            end = child.start_byte();
                            break;
                        }
                        "ordered_field_declaration_list" | ";" => {
                            end = child.end_byte();
                            break;
                        }
                        _ => {}
                    }
                }
                end
            };
            let signature = flatten_bytes(start_byte, end_byte, source)
                .unwrap_or_else(|| first_line_of_node(node, source));
            let start_row = attrs
                .first()
                .map(|a| a.start_position().row + 1)
                .unwrap_or_else(|| node.start_position().row + 1);
            let [start, end] = line_range(start_row, node);
            results.push(DefContent {
                kind: def_kind,
                lines: [start, end],
                signature,
                scope,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::extract_definitions;

    // --- Disambiguation / kind filter edge cases ---

    #[test]
    fn enum_not_matched_by_struct() {
        let src = "enum Foo { A } struct Foo { x: i32 }";
        let results = extract_definitions(&RustParser, "Foo", &[DefKind::Enum], src);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].kind, DefKind::Enum);
    }

    #[test]
    fn const_fn_is_function_not_const() {
        let src = "const fn factorial(n: u64) -> u64 { 1 }";
        let results = extract_definitions(&RustParser, "factorial", &[DefKind::Const], src);
        assert!(results.is_empty());
        let results = extract_definitions(&RustParser, "factorial", &[DefKind::Function], src);
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn macro_kind_filter() {
        let src = "macro_rules! foo { () => {}; } fn foo() {}";
        let results = extract_definitions(&RustParser, "foo", &[DefKind::Macro], src);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].kind, DefKind::Macro);
        let results = extract_definitions(&RustParser, "foo", &[DefKind::Function], src);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].kind, DefKind::Function);
    }

    #[test]
    fn proc_macro_is_function_not_macro() {
        let src = "#[proc_macro]\npub fn sql(input: TokenStream) -> TokenStream { input }";
        let results = extract_definitions(&RustParser, "sql", &[DefKind::Function], src);
        assert_eq!(results.len(), 1);
        let results = extract_definitions(&RustParser, "sql", &[DefKind::Macro], src);
        assert!(results.is_empty());
    }

    // --- Trait method edge cases ---

    #[test]
    fn trait_required_method_found_as_function() {
        let src = "trait MyTrait { fn required_method(&self) -> i32; }";
        let results =
            extract_definitions(&RustParser, "required_method", &[DefKind::Function], src);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].kind, DefKind::Function);
        assert_eq!(results[0].scope, "MyTrait::required_method");
    }

    #[test]
    fn trait_provided_method_found_as_function() {
        let src = "trait MyTrait { fn provided(&self) -> i32 { 42 } }";
        let results = extract_definitions(&RustParser, "provided", &[DefKind::Function], src);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].scope, "MyTrait::provided");
    }

    #[test]
    fn trait_method_with_generics() {
        let src = "trait MyTrait { fn convert<U>(&self) -> U; }";
        let results = extract_definitions(&RustParser, "convert", &[DefKind::Function], src);
        assert_eq!(results.len(), 1);
        assert!(results[0].signature.contains("<U>"));
    }

    #[test]
    fn trait_method_with_attr() {
        let src = "trait MyTrait { #[inline] fn tagged(&self) -> i32; }";
        let results = extract_definitions(&RustParser, "tagged", &[DefKind::Function], src);
        assert_eq!(results.len(), 1);
        assert!(results[0].signature.contains("#[inline]"));
    }

    #[test]
    fn nested_mod_trait_method() {
        let src = "mod m { trait T { fn f(&self); } }";
        let results = extract_definitions(&RustParser, "f", &[DefKind::Function], src);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].scope, "m::T::f");
    }

    // --- Function-body definitions should NOT be extracted ---

    #[test]
    fn fn_inside_fn_not_extracted() {
        let src = "fn outer() { fn inner() {} }";
        let results = extract_definitions(&RustParser, "inner", &[DefKind::Function], src);
        assert!(
            results.is_empty(),
            "Function-body definitions should not be extracted, got: {results:?}"
        );
    }

    #[test]
    fn struct_inside_fn_not_extracted() {
        let src = "fn outer() { struct Inner {} }";
        let results = extract_definitions(&RustParser, "Inner", &[DefKind::Struct], src);
        assert!(
            results.is_empty(),
            "Function-body definitions should not be extracted, got: {results:?}"
        );
    }

    #[test]
    fn enum_inside_fn_not_extracted() {
        let src = "fn outer() { enum Inner { A } }";
        let results = extract_definitions(&RustParser, "Inner", &[DefKind::Enum], src);
        assert!(
            results.is_empty(),
            "Function-body definitions should not be extracted, got: {results:?}"
        );
    }

    #[test]
    fn type_inside_fn_not_extracted() {
        let src = "fn outer() { type Inner = i32; }";
        let results = extract_definitions(&RustParser, "Inner", &[DefKind::Type], src);
        assert!(
            results.is_empty(),
            "Function-body definitions should not be extracted, got: {results:?}"
        );
    }

    #[test]
    fn impl_inside_fn_not_extracted() {
        let src = "fn outer() { struct S {} impl S { fn method() {} } }";
        let results = extract_definitions(&RustParser, "method", &[DefKind::Function], src);
        assert!(
            results.is_empty(),
            "Function-body definitions should not be extracted, got: {results:?}"
        );
    }

    // --- Module kind tests ---

    #[test]
    fn mod_item_extracted_as_module() {
        let src = "mod my_mod {}";
        let results = extract_definitions(&RustParser, "my_mod", &[DefKind::Module], src);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].kind, DefKind::Module);
        assert_eq!(results[0].scope, "my_mod");
    }

    #[test]
    fn mod_item_bodyless_extracted() {
        let src = "mod my_mod;";
        let results = extract_definitions(&RustParser, "my_mod", &[DefKind::Module], src);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].kind, DefKind::Module);
    }

    #[test]
    fn nested_mod_scope() {
        let src = "mod outer { mod inner { fn deep() {} } }";
        let results = extract_definitions(&RustParser, "deep", &[DefKind::Function], src);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].scope, "outer::inner::deep");
    }

    #[test]
    fn mod_inside_fn_not_extracted() {
        let src = "fn outer() { mod inner_mod {} }";
        let results = extract_definitions(&RustParser, "inner_mod", &[DefKind::Module], src);
        assert!(
            results.is_empty(),
            "Function-body definitions should not be extracted, got: {results:?}"
        );
    }

    #[test]
    fn mod_with_attrs() {
        let src = "#[cfg(test)] mod test_mod {}";
        let results = extract_definitions(&RustParser, "test_mod", &[DefKind::Module], src);
        assert_eq!(results.len(), 1);
        assert!(results[0].signature.contains("#[cfg(test)]"));
    }

    // --- extract_impl_type regression ---

    #[test]
    fn impl_for_generic_type_scope() {
        let src = "struct Foo<T> {} impl<T> Foo<T> { fn new() {} }";
        let results = extract_definitions(&RustParser, "new", &[DefKind::Function], src);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].scope, "Foo::new");
    }
}
