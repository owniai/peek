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
            DefKind::Method,
            DefKind::Struct,
            DefKind::Enum,
            DefKind::Alias,
            DefKind::Trait,
            DefKind::Const,
            DefKind::Macro,
            DefKind::Module,
            DefKind::Union,
            DefKind::Field,
            DefKind::Static,
            DefKind::Variant,
        ]
    }

    impl_init_parser!(tree_sitter_rust::LANGUAGE, "Rust");

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

fn kind_for_node(node: Node) -> Option<DefKind> {
    match node.kind() {
        "function_item" => Some(DefKind::Function),
        "function_signature_item" => Some(DefKind::Function),
        "struct_item" => Some(DefKind::Struct),
        "union_item" => Some(DefKind::Union),
        "enum_item" => Some(DefKind::Enum),
        "type_item" => Some(DefKind::Alias),
        "trait_item" => Some(DefKind::Trait),
        "const_item" => Some(DefKind::Const),
        "static_item" => Some(DefKind::Static),
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
    in_type_body: bool,
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
                try_add_definition(
                    child,
                    source,
                    mode,
                    kinds,
                    results,
                    scope,
                    &attr_buffer,
                    in_type_body,
                );
                attr_buffer.clear();
                // Don't recurse into function body — function-scoped definitions are not extracted
            }
            "impl_item" => {
                attr_buffer.clear();
                let new_scope = compute_impl_scope(child, source, scope);
                let mut inner_cursor = child.walk();
                for grandchild in child.children(&mut inner_cursor) {
                    collect_definitions(grandchild, source, mode, kinds, results, &new_scope, true);
                }
            }
            "field_declaration" => {
                handle_field(child, source, mode, kinds, results, scope);
                attr_buffer.clear();
            }
            "enum_variant" => {
                handle_variant(child, source, mode, kinds, results, scope, &attr_buffer);
                attr_buffer.clear();
            }
            _ => {
                let is_type_body = child.kind() == "trait_item";
                try_add_definition(
                    child,
                    source,
                    mode,
                    kinds,
                    results,
                    scope,
                    &attr_buffer,
                    in_type_body,
                );
                attr_buffer.clear();
                let child_scope = match child.kind() {
                    "trait_item" | "mod_item" | "struct_item" | "union_item" | "enum_item" => {
                        let name = extract_name(child, source).unwrap_or_default();
                        build_scope(scope, "::", &name)
                    }
                    _ => scope.to_string(),
                };
                // Preserve in_type_body through non-type containers (e.g. declaration_list),
                // only set to true when entering a new type body (trait_item)
                let child_in_type_body = in_type_body || is_type_body;
                collect_definitions(
                    child,
                    source,
                    mode,
                    kinds,
                    results,
                    &child_scope,
                    child_in_type_body,
                );
            }
        }
    }
}

fn handle_field<'a>(
    node: Node<'a>,
    source: &str,
    mode: &MatchMode,
    kinds: &[DefKind],
    results: &mut Vec<DefContent>,
    scope: &str,
) {
    if !kinds.contains(&DefKind::Field) {
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
    let field_scope = build_scope(scope, "::", &name);
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

fn handle_variant<'a>(
    node: Node<'a>,
    source: &str,
    mode: &MatchMode,
    kinds: &[DefKind],
    results: &mut Vec<DefContent>,
    scope: &str,
    attrs: &[Node<'a>],
) {
    if !kinds.contains(&DefKind::Variant) {
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
    let variant_scope = build_scope(scope, "::", &name);
    let start_byte = attrs
        .first()
        .map(|a| a.start_byte())
        .unwrap_or_else(|| node.start_byte());
    let end_byte = {
        let mut cursor = node.walk();
        let mut end = node.end_byte();
        for child in node.children(&mut cursor) {
            match child.kind() {
                "enum_variant_list" | "field_declaration_list" | "block" => {
                    end = child.start_byte();
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
        kind: DefKind::Variant,
        lines: [start, end],
        signature,
        scope: variant_scope,
    });
}

#[allow(clippy::too_many_arguments)]
fn try_add_definition<'a>(
    node: Node<'a>,
    source: &str,
    mode: &MatchMode,
    kinds: &[DefKind],
    results: &mut Vec<DefContent>,
    scope: &str,
    attrs: &[Node<'a>],
    in_type_body: bool,
) {
    let def_kind = match kind_for_node(node) {
        Some(k) => k,
        None => return,
    };

    // In impl/trait body, function_item and function_signature_item become Method
    let def_kind = if in_type_body && def_kind == DefKind::Function {
        DefKind::Method
    } else {
        def_kind
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

    // --- Trait/impl method edge cases ---

    #[test]
    fn trait_required_method_found_as_method() {
        let src = "trait MyTrait { fn required_method(&self) -> i32; }";
        let results = extract_definitions(&RustParser, "required_method", &[DefKind::Method], src);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].kind, DefKind::Method);
        assert_eq!(results[0].scope, "MyTrait::required_method");
    }

    #[test]
    fn trait_provided_method_found_as_method() {
        let src = "trait MyTrait { fn provided(&self) -> i32 { 42 } }";
        let results = extract_definitions(&RustParser, "provided", &[DefKind::Method], src);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].kind, DefKind::Method);
        assert_eq!(results[0].scope, "MyTrait::provided");
    }

    #[test]
    fn trait_method_with_generics() {
        let src = "trait MyTrait { fn convert<U>(&self) -> U; }";
        let results = extract_definitions(&RustParser, "convert", &[DefKind::Method], src);
        assert_eq!(results.len(), 1);
        assert!(results[0].signature.contains("<U>"));
    }

    #[test]
    fn trait_method_with_attr() {
        let src = "trait MyTrait { #[inline] fn tagged(&self) -> i32; }";
        let results = extract_definitions(&RustParser, "tagged", &[DefKind::Method], src);
        assert_eq!(results.len(), 1);
        assert!(results[0].signature.contains("#[inline]"));
    }

    #[test]
    fn nested_mod_trait_method() {
        let src = "mod m { trait T { fn f(&self); } }";
        let results = extract_definitions(&RustParser, "f", &[DefKind::Method], src);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].scope, "m::T::f");
    }

    #[test]
    fn impl_method_is_method_kind() {
        let src = "struct S {} impl S { fn new() {} }";
        let results = extract_definitions(&RustParser, "new", &[DefKind::Method], src);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].kind, DefKind::Method);
        assert_eq!(results[0].scope, "S::new");
    }

    #[test]
    fn impl_method_not_matched_by_function_kind() {
        let src = "struct S {} impl S { fn new() {} }";
        let results = extract_definitions(&RustParser, "new", &[DefKind::Function], src);
        assert!(
            results.is_empty(),
            "impl method should not match -k function, got: {results:?}"
        );
    }

    #[test]
    fn top_level_fn_still_function_kind() {
        let src = "fn top_fn() {}";
        let results = extract_definitions(&RustParser, "top_fn", &[DefKind::Function], src);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].kind, DefKind::Function);
    }

    #[test]
    fn callable_kind_matches_both_function_and_method() {
        let src = "fn top_fn() {} struct S {} impl S { fn method() {} }";
        let results =
            extract_definitions(&RustParser, ".", &[DefKind::Function, DefKind::Method], src);
        assert_eq!(results.len(), 2);
        let kinds: Vec<DefKind> = results.iter().map(|r| r.kind).collect();
        assert!(kinds.contains(&DefKind::Function));
        assert!(kinds.contains(&DefKind::Method));
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
        let results = extract_definitions(&RustParser, "Inner", &[DefKind::Alias], src);
        assert!(
            results.is_empty(),
            "Function-body definitions should not be extracted, got: {results:?}"
        );
    }

    #[test]
    fn impl_inside_fn_not_extracted() {
        let src = "fn outer() { struct S {} impl S { fn method() {} } }";
        let results = extract_definitions(&RustParser, "method", &[DefKind::Method], src);
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
        let results = extract_definitions(&RustParser, "new", &[DefKind::Method], src);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].scope, "Foo::new");
    }

    // --- Union ---

    #[test]
    fn union_extracted_as_union_kind() {
        let src = "union IntOrFloat { i: i32, f: f32, }";
        let results = extract_definitions(&RustParser, "IntOrFloat", &[DefKind::Union], src);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].kind, DefKind::Union);
        assert_eq!(results[0].scope, "IntOrFloat");
        assert!(results[0].signature.contains("union IntOrFloat"));
    }

    #[test]
    fn union_not_matched_by_struct() {
        let src = "union U { i: i32 } struct S { x: i32 }";
        let results = extract_definitions(&RustParser, "U", &[DefKind::Struct], src);
        assert!(results.is_empty());
        let results = extract_definitions(&RustParser, "U", &[DefKind::Union], src);
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn union_with_generics() {
        let src = "union TaggedValue<T> { int_val: i64, _phantom: std::marker::PhantomData<T>, }";
        let results = extract_definitions(&RustParser, "TaggedValue", &[DefKind::Union], src);
        assert_eq!(results.len(), 1);
        assert!(results[0].signature.contains("<T>"));
    }

    #[test]
    fn union_with_attr() {
        let src = "#[repr(C)] pub union Data { i: i32, f: f32, }";
        let results = extract_definitions(&RustParser, "Data", &[DefKind::Union], src);
        assert_eq!(results.len(), 1);
        assert!(results[0].signature.contains("#[repr(C)]"));
        assert!(results[0].signature.contains("pub"));
    }

    // --- Field extraction ---

    #[test]
    fn struct_field_extracted_as_field() {
        let src = "struct Point { x: i32, y: i32 }";
        let results = extract_definitions(&RustParser, "x", &[DefKind::Field], src);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].kind, DefKind::Field);
        assert_eq!(results[0].scope, "Point::x");
    }

    #[test]
    fn struct_multiple_fields() {
        let src = "struct Point { x: i32, y: i32 }";
        let results = extract_definitions(&RustParser, "y", &[DefKind::Field], src);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].kind, DefKind::Field);
        assert_eq!(results[0].scope, "Point::y");
    }

    #[test]
    fn field_kind_filter_excludes_struct() {
        let src = "struct Foo { bar: i32 }";
        let results = extract_definitions(&RustParser, "bar", &[DefKind::Struct], src);
        assert!(
            results.is_empty(),
            "field should not match -k struct, got: {results:?}"
        );
    }

    #[test]
    fn field_kind_filter_excludes_method() {
        let src = "struct S {} impl S { fn bar(&self) {} }";
        let results = extract_definitions(&RustParser, "bar", &[DefKind::Field], src);
        assert!(
            results.is_empty(),
            "method should not match -k field, got: {results:?}"
        );
    }

    // --- Static extraction ---

    #[test]
    fn static_item_extracted_as_static() {
        let src = "static MAX_SIZE: usize = 100;";
        let results = extract_definitions(&RustParser, "MAX_SIZE", &[DefKind::Static], src);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].kind, DefKind::Static);
        assert_eq!(results[0].scope, "MAX_SIZE");
    }

    #[test]
    fn static_mut_extracted_as_static() {
        let src = "static mut COUNT: i32 = 0;";
        let results = extract_definitions(&RustParser, "COUNT", &[DefKind::Static], src);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].kind, DefKind::Static);
        assert_eq!(results[0].scope, "COUNT");
    }

    #[test]
    fn static_kind_filter_excludes_const() {
        let src = "static VALUE: i32 = 42; const CONST_VAL: i32 = 1;";
        let results = extract_definitions(&RustParser, "VALUE", &[DefKind::Const], src);
        assert!(
            results.is_empty(),
            "static should not match -k const, got: {results:?}"
        );
        let results = extract_definitions(&RustParser, "CONST_VAL", &[DefKind::Static], src);
        assert!(
            results.is_empty(),
            "const should not match -k static, got: {results:?}"
        );
    }

    #[test]
    fn static_inside_mod_scope() {
        let src = "mod m { static INNER: i32 = 0; }";
        let results = extract_definitions(&RustParser, "INNER", &[DefKind::Static], src);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].scope, "m::INNER");
    }

    #[test]
    fn field_inside_fn_not_extracted() {
        let src = "fn outer() { struct S { x: i32 } }";
        let results = extract_definitions(&RustParser, "x", &[DefKind::Field], src);
        assert!(
            results.is_empty(),
            "Function-body field should not be extracted, got: {results:?}"
        );
    }
}
