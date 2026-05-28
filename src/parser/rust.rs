use crate::model::{DefContent, DefKind};
use crate::parser::{
    LanguageParser, MatchMode, build_scope, first_line_of_node, flatten_bytes, line_range,
    node_text, node_text_ref,
};
use tree_sitter::{Node, Parser};

pub(crate) const LANGUAGE: &str = "rust";
pub(crate) const EXTENSIONS: &[&str] = &["rs"];
pub(crate) const ALIASES: &[&str] = &["rs"];

pub struct RustParser;

impl LanguageParser for RustParser {
    fn language(&self) -> &'static str {
        LANGUAGE
    }

    fn extensions(&self) -> &'static [&'static str] {
        EXTENSIONS
    }

    fn supported_kinds(&self) -> &'static [DefKind] {
        &[
            DefKind::Function,
            DefKind::FunctionDeclaration,
            DefKind::Method,
            DefKind::MethodDeclaration,
            DefKind::Struct,
            DefKind::Enum,
            DefKind::Alias,
            DefKind::AssociatedType,
            DefKind::Trait,
            DefKind::Const,
            DefKind::Macro,
            DefKind::Module,
            DefKind::ModuleDeclaration,
            DefKind::Union,
            DefKind::Field,
            DefKind::Var,
            DefKind::Variant,
            DefKind::Operator,
            DefKind::OperatorDeclaration,
            DefKind::Subscript,
            DefKind::SubscriptDeclaration,
            DefKind::Destructor,
            DefKind::DestructorDeclaration,
            DefKind::Impl,
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
            None,
            false,
        );
        Ok(results)
    }
}

fn kind_for_node(node: Node) -> Option<DefKind> {
    match node.kind() {
        "function_item" => Some(DefKind::Function),
        "function_signature_item" => Some(DefKind::FunctionDeclaration),
        "struct_item" => Some(DefKind::Struct),
        "union_item" => Some(DefKind::Union),
        "enum_item" => Some(DefKind::Enum),
        "type_item" => Some(DefKind::Alias),
        "associated_type" => Some(DefKind::AssociatedType),
        "trait_item" => Some(DefKind::Trait),
        "const_item" => Some(DefKind::Const),
        "static_item" => {
            let has_mut = node
                .children(&mut node.walk())
                .any(|c| c.kind() == "mutable_specifier");
            if has_mut {
                Some(DefKind::Var)
            } else {
                Some(DefKind::Const)
            }
        }
        "macro_definition" => Some(DefKind::Macro),
        "mod_item" => {
            // mod foo; (bodyless) → ModuleDeclaration; mod foo { ... } (with body) → Module
            let has_body = node.child_by_field_name("body").is_some();
            if has_body {
                Some(DefKind::Module)
            } else {
                Some(DefKind::ModuleDeclaration)
            }
        }
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

fn extract_trait_name<'a>(trait_node: Node, source: &'a str) -> Option<&'a str> {
    let text = node_text_ref(trait_node, source);
    let base = text.split('<').next().unwrap_or(text);
    let name = base.rsplit("::").next().unwrap_or(base);
    if name.is_empty() { None } else { Some(name) }
}

// Classify trait impl methods by the short trait name.
// Operator: all traits in std::ops (arithmetic, bitwise, unary, compound assignment, deref, call)
// plus comparison traits from std::cmp.
// Subscript: Index/IndexMut (enables [] syntax, aligns with Swift/C# subscript).
// Destructor: Drop (enables RAII cleanup, aligns with C++/Swift destructor).
// Non-matching traits (Display, Clone, etc.) return None → methods stay as Method.
fn classify_trait_method(trait_name: &str) -> Option<DefKind> {
    match trait_name {
        // std::ops — arithmetic & bitwise & unary
        "Add" | "Sub" | "Mul" | "Div" | "Rem"
        | "Neg" | "Not"
        | "BitAnd" | "BitOr" | "BitXor" | "Shl" | "Shr"
        // std::ops — compound assignment
        | "AddAssign" | "SubAssign" | "MulAssign" | "DivAssign" | "RemAssign"
        | "BitAndAssign" | "BitOrAssign" | "BitXorAssign" | "ShlAssign" | "ShrAssign"
        // std::ops — deref (overloads * syntax, also used for Deref coercion)
        | "Deref" | "DerefMut"
        // std::cmp — comparison (overloads == != < > <= >=)
        | "PartialEq" | "Eq" | "PartialOrd" | "Ord"
        // std::ops — callable (overloads () syntax)
        | "Fn" | "FnMut" | "FnOnce" => Some(DefKind::Operator),
        "Index" | "IndexMut" => Some(DefKind::Subscript),
        "Drop" => Some(DefKind::Destructor),
        _ => None,
    }
}

#[allow(clippy::too_many_arguments)]
fn collect_definitions<'a>(
    node: Node<'a>,
    source: &str,
    mode: &MatchMode,
    kinds: &[DefKind],
    results: &mut Vec<DefContent>,
    scope: &str,
    in_type_body: bool,
    trait_kind: Option<DefKind>,
    in_trait_body: bool,
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
                    trait_kind,
                    in_trait_body,
                );
                attr_buffer.clear();
                // Don't recurse into function body — function-scoped definitions are not extracted
            }
            "impl_item" => {
                attr_buffer.clear();
                let new_scope = compute_impl_scope(child, source, scope);
                let trait_name_short = child
                    .child_by_field_name("trait")
                    .and_then(|n| extract_trait_name(n, source));
                let new_trait_kind = trait_name_short.and_then(classify_trait_method);

                if kinds.contains(&DefKind::Impl) {
                    let impl_scope = match trait_name_short {
                        Some(tn) => format!("{tn} for {new_scope}"),
                        None => new_scope.clone(),
                    };
                    if mode.matches_ident(&impl_scope) {
                        let sig_end = {
                            let mut cur = child.walk();
                            let mut end = child.end_byte();
                            for gc in child.children(&mut cur) {
                                if gc.kind() == "declaration_list" {
                                    end = gc.start_byte();
                                    break;
                                }
                            }
                            end
                        };
                        let signature = flatten_bytes(child.start_byte(), sig_end, source)
                            .unwrap_or_else(|| first_line_of_node(child, source));
                        let [start, end] = line_range(child.start_position().row + 1, child);
                        results.push(DefContent {
                            kind: DefKind::Impl,
                            lines: [start, end],
                            signature,
                            scope: impl_scope,
                        });
                    }
                }

                let mut inner_cursor = child.walk();
                for grandchild in child.children(&mut inner_cursor) {
                    collect_definitions(
                        grandchild,
                        source,
                        mode,
                        kinds,
                        results,
                        &new_scope,
                        true,
                        new_trait_kind,
                        false,
                    );
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
                    None,
                    in_trait_body || is_type_body,
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
                    None,
                    in_trait_body || is_type_body,
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
        scope: variant_scope.clone(),
    });

    // Recurse into record variant fields (e.g. Rgb { r: u8, g: u8, b: u8 })
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "field_declaration_list" {
            let mut field_cursor = child.walk();
            for field_node in child.children(&mut field_cursor) {
                if field_node.kind() == "field_declaration" {
                    handle_field(field_node, source, mode, kinds, results, &variant_scope);
                }
            }
        }
    }
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
    trait_kind: Option<DefKind>,
    _in_trait_body: bool,
) {
    let def_kind = match kind_for_node(node) {
        Some(k) => k,
        None => return,
    };

    // Proc macro functions (#[proc_macro], #[proc_macro_derive], #[proc_macro_attribute])
    // are Macro definitions, not Function
    let def_kind = if def_kind == DefKind::Function && node.kind() == "function_item" {
        let is_proc_macro = attrs.iter().any(|a| {
            let text = node_text_ref(*a, source);
            text.starts_with("#[proc_macro")
        });
        if is_proc_macro {
            DefKind::Macro
        } else {
            def_kind
        }
    } else {
        def_kind
    };

    // In impl/trait body, function_item and function_signature_item become Method
    // or a more specific callable sub-kind based on the trait being implemented
    let def_kind = if in_type_body && def_kind == DefKind::Function {
        trait_kind.unwrap_or(DefKind::Method)
    } else if in_type_body && def_kind == DefKind::FunctionDeclaration {
        trait_kind
            .unwrap_or(DefKind::Method)
            .declaration_pair()
            .expect("callable kinds always have declaration pairs")
    } else if in_type_body && def_kind == DefKind::Alias {
        // tree-sitter-rust parses `type X = ...;` as type_item (both in traits and impls),
        // but semantically these are associated types (trait declarations with defaults,
        // or impl provisions fulfilling the trait's associated type requirement).
        DefKind::AssociatedType
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
    use crate::model::DefKind;

    fn extract(source: &str) -> Vec<DefContent> {
        let parser = RustParser;
        let mut ts_parser = parser.init_parser();
        let all_kinds: Vec<DefKind> = parser.supported_kinds().to_vec();
        parser
            .extract_with(&MatchMode::All, &all_kinds, source, &mut ts_parser)
            .unwrap()
    }

    #[test]
    fn attribute_on_function_sets_start_row() {
        let source = "\n#[inline]\nfn foo() {}\n";
        let defs = extract(source);
        let foo = defs.iter().find(|d| d.scope == "foo").unwrap();
        assert_eq!(
            foo.lines[0], 2,
            "start line should be the #[inline] attribute line"
        );
    }

    #[test]
    fn attribute_on_struct_sets_start_row() {
        let source = "\n#[derive(Debug)]\nstruct Foo { x: i32 }\n";
        let defs = extract(source);
        let foo = defs.iter().find(|d| d.scope == "Foo").unwrap();
        assert_eq!(
            foo.lines[0], 2,
            "start line should be the #[derive] attribute line"
        );
    }

    #[test]
    fn no_attribute_uses_node_start_row() {
        let source = "\nfn bar() {}\n";
        let defs = extract(source);
        let bar = defs.iter().find(|d| d.scope == "bar").unwrap();
        assert_eq!(bar.lines[0], 2, "start line should be the function line");
    }

    #[test]
    fn attribute_on_enum_variant_sets_start_row() {
        let source = "\nenum E {\n  #[default]\n  A,\n  B,\n}\n";
        let defs = extract(source);
        let a = defs.iter().find(|d| d.scope == "E::A").unwrap();
        assert_eq!(
            a.lines[0], 3,
            "start line should be the #[default] attribute line"
        );
    }
}
