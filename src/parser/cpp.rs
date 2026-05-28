use crate::model::{DefContent, DefKind};
use crate::parser::{
    LanguageParser, MatchMode, build_scope, build_scope_from_node, extract_const_name,
    extract_function_name, extract_signature_to_body, extract_typedef_name, extract_var_name,
    first_line_of_node, handle_macro, has_extern_storage_class, has_function_declarator,
    has_initializer, is_const_declaration, line_range, node_text, node_text_ref,
};
use tree_sitter::{Node, Parser};

pub(crate) const LANGUAGE: &str = "cplusplus";
pub(crate) const EXTENSIONS: &[&str] = &[
    "cpp", "cxx", "cc", "hpp", "hxx", "hh", "h", "ixx", "cppm", "inl", "tcc", "tpp", "ipp", "inc",
    "ino", "txx",
];
pub(crate) const ALIASES: &[&str] = &["cpp", "c++", "cxx"];

pub struct CppParser;

impl LanguageParser for CppParser {
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
            DefKind::Constructor,
            DefKind::ConstructorDeclaration,
            DefKind::Class,
            DefKind::ClassDeclaration,
            DefKind::Struct,
            DefKind::StructDeclaration,
            DefKind::Union,
            DefKind::UnionDeclaration,
            DefKind::Enum,
            DefKind::EnumDeclaration,
            DefKind::Linkage,
            DefKind::Alias,
            DefKind::Const,
            DefKind::ConstDeclaration,
            DefKind::Macro,
            DefKind::Field,
            DefKind::Var,
            DefKind::VarDeclaration,
            DefKind::Namespace,
            DefKind::Variant,
            DefKind::Concept,
            DefKind::Destructor,
            DefKind::DestructorDeclaration,
            DefKind::Operator,
            DefKind::OperatorDeclaration,
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
        "linkage_specification" => {
            let name = node
                .child_by_field_name("value")
                .map(|v| node_text_ref(v, source).trim_matches('"').to_string())
                .unwrap_or_default();
            let new_scope = build_scope(scope, "::", &name);
            if kinds.contains(&DefKind::Linkage) && mode.matches_ident(&new_scope) {
                let sig = extract_signature_to_body(node, source);
                let start_row = node.start_position().row + 1;
                let [start, end] = line_range(start_row, node);
                results.push(DefContent {
                    kind: DefKind::Linkage,
                    lines: [start, end],
                    signature: sig,
                    scope: new_scope.clone(),
                });
            }
            recurse_into_body(node, source, mode, kinds, results, &new_scope, false);
            return;
        }
        "class_specifier" => {
            handle_class_or_struct(node, source, mode, kinds, results, scope, DefKind::Class);
            let new_scope = scope_for_anon_type_in_field(node, source, scope);
            recurse_into_body(node, source, mode, kinds, results, &new_scope, true);
            return;
        }
        "struct_specifier" => {
            handle_class_or_struct(node, source, mode, kinds, results, scope, DefKind::Struct);
            let new_scope = scope_for_anon_type_in_field(node, source, scope);
            recurse_into_body(node, source, mode, kinds, results, &new_scope, true);
            return;
        }
        "union_specifier" => {
            handle_class_or_struct(node, source, mode, kinds, results, scope, DefKind::Union);
            let new_scope = scope_for_anon_type_in_field(node, source, scope);
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
                    // Anonymous struct/union in typedef: use typedef name for scope
                    // so inner fields get correct scoping like AnonT::x
                    let inner_scope = if type_node.child_by_field_name("name").is_none() {
                        if let Some(declarator) = node.child_by_field_name("declarator") {
                            if let Some(name) = extract_typedef_name(declarator, source) {
                                build_scope(scope, "::", &name)
                            } else {
                                scope.to_string()
                            }
                        } else {
                            scope.to_string()
                        }
                    } else {
                        scope.to_string()
                    };
                    recurse_into_body(type_node, source, mode, kinds, results, &inner_scope, true);
                } else if type_node.kind() == "enum_specifier" {
                    let enum_scope =
                        if let Some(declarator) = node.child_by_field_name("declarator") {
                            if let Some(name) = extract_typedef_name(declarator, source) {
                                build_scope(scope, "::", &name)
                            } else {
                                scope.to_string()
                            }
                        } else {
                            scope.to_string()
                        };
                    recurse_into_body(type_node, source, mode, kinds, results, &enum_scope, true);
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
        "declaration" if has_function_declarator(node) => {
            handle_function(node, source, mode, kinds, results, scope, in_type_body);
            return;
        }
        "declaration" => {
            // Catch-all: non-const, non-function declaration → Var
            handle_var(node, source, mode, kinds, results, scope);
            return;
        }
        "field_declaration" => {
            if has_function_declarator(node) {
                handle_function(node, source, mode, kinds, results, scope, in_type_body);
            } else {
                handle_field(node, source, mode, kinds, results, scope);
            }
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
        "concept_definition" => {
            handle_concept(node, source, mode, kinds, results, scope);
            return;
        }
        "namespace_alias_definition" => {
            handle_namespace_alias(node, source, mode, kinds, results, scope);
            return;
        }
        "using_declaration" if !is_using_directive(node) => {
            handle_using_declaration(node, source, mode, kinds, results, scope);
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

    // Detect if this is a declaration (no body) vs definition (has body).
    // = default and = delete have no body but are considered definitions per design.
    let has_body = node.child_by_field_name("body").is_some();
    let is_default_or_delete = has_default_or_delete_clause(node);
    let is_declaration = !has_body && !is_default_or_delete;

    // Check if this is a destructor (contains destructor_name in declarator chain)
    let destructor_name = find_destructor_name(declarator, source);
    if let Some(dtor_text) = &destructor_name {
        let def_kind = if is_declaration {
            DefKind::DestructorDeclaration
        } else {
            DefKind::Destructor
        };
        if !kind_matches(kinds, def_kind) {
            return;
        }
        if !mode.matches_ident(dtor_text) {
            return;
        }
        let signature = extract_signature_to_body(node, source);
        let start_row = node.start_position().row + 1;
        let [start, end] = line_range(start_row, node);
        results.push(DefContent {
            kind: def_kind,
            lines: [start, end],
            signature,
            scope: build_scope(scope, "::", dtor_text),
        });
        return;
    }

    // Check if this is an operator overload (contains operator_name in declarator chain)
    if let Some(op_text) = find_operator_name(declarator, source) {
        let def_kind = if is_declaration {
            DefKind::OperatorDeclaration
        } else {
            DefKind::Operator
        };
        if !kind_matches(kinds, def_kind) {
            return;
        }
        if !mode.matches_ident(&op_text) {
            return;
        }
        let qualified = qualified_name_from_declarator(declarator, source);
        let scope_name = qualified.as_deref().unwrap_or(&op_text);
        let signature = extract_signature_to_body(node, source);
        let start_row = node.start_position().row + 1;
        let [start, end] = line_range(start_row, node);
        results.push(DefContent {
            kind: def_kind,
            lines: [start, end],
            signature,
            scope: build_scope(scope, "::", scope_name),
        });
        return;
    }

    let name_text = match extract_function_name(declarator, source) {
        Some(n) => n,
        None => return,
    };

    let qualified = qualified_name_from_declarator(declarator, source);
    // Constructor: no return type (destructors/operators already handled above)
    let base_kind = if node.child_by_field_name("type").is_none() {
        DefKind::Constructor
    } else if in_type_body || qualified.is_some() {
        DefKind::Method
    } else {
        DefKind::Function
    };

    let def_kind = if is_declaration {
        to_declaration_kind(base_kind)
    } else {
        base_kind
    };

    if !kind_matches(kinds, def_kind) {
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

/// Check if a node has a `default_method_clause` or `delete_method_clause` child.
///
/// In C++ tree-sitter, `= default` and `= delete` produce these child nodes
/// on `function_definition` nodes. They have no body but are considered
/// definitions per the design requirement.
fn has_default_or_delete_clause(node: Node) -> bool {
    let mut cursor = node.walk();
    node.children(&mut cursor)
        .any(|c| matches!(c.kind(), "default_method_clause" | "delete_method_clause"))
}

/// Convert a definition kind to its declaration variant.
///
/// Maps Function -> FunctionDeclaration, Method -> MethodDeclaration, etc.
/// Returns the same kind if no declaration variant exists (should not happen
/// for callable kinds used in C++).
fn to_declaration_kind(kind: DefKind) -> DefKind {
    kind.declaration_pair().unwrap_or(kind)
}

/// Check if the requested kinds include the given kind.
///
/// Simple exact match. The kinds_from_tag expansion at the model layer handles
/// bidirectional matching: `-k method` expands to [Method, MethodDeclaration],
/// while `-k method_declaration` stays as [MethodDeclaration].
fn kind_matches(kinds: &[DefKind], kind: DefKind) -> bool {
    kinds.contains(&kind)
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
    let has_body = node.child_by_field_name("body").is_some();
    let kind = if has_body {
        if !kinds.contains(&def_kind) {
            return;
        }
        def_kind
    } else {
        let Some(decl_kind) = def_kind.declaration_pair() else {
            return;
        };
        if !kinds.contains(&decl_kind) {
            return;
        }
        decl_kind
    };

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
        kind,
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
    let has_body = node.child_by_field_name("body").is_some();
    let kind = if has_body {
        if !kinds.contains(&DefKind::Enum) {
            return;
        }
        DefKind::Enum
    } else {
        if !kinds.contains(&DefKind::EnumDeclaration) {
            return;
        }
        DefKind::EnumDeclaration
    };

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
        kind,
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
                "enum_specifier" => DefKind::Enum,
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
    let is_extern_decl = has_extern_storage_class(node, source) && !has_initializer(node);
    let kind = if is_extern_decl {
        if !kinds.contains(&DefKind::ConstDeclaration) {
            return;
        }
        DefKind::ConstDeclaration
    } else {
        if !kinds.contains(&DefKind::Const) {
            return;
        }
        DefKind::Const
    };

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
        kind,
        lines: [start, end],
        signature,
        scope: build_scope(scope, "::", &name_text),
    });
}

fn handle_var(
    node: Node,
    source: &str,
    mode: &MatchMode,
    kinds: &[DefKind],
    results: &mut Vec<DefContent>,
    scope: &str,
) {
    let is_extern_decl = has_extern_storage_class(node, source) && !has_initializer(node);
    let kind = if is_extern_decl {
        if !kinds.contains(&DefKind::VarDeclaration) {
            return;
        }
        DefKind::VarDeclaration
    } else {
        if !kinds.contains(&DefKind::Var) {
            return;
        }
        DefKind::Var
    };

    let declarator = match node.child_by_field_name("declarator") {
        Some(d) => d,
        None => return,
    };
    let name_text = match extract_var_name(declarator, source) {
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

fn handle_concept(
    node: Node,
    source: &str,
    mode: &MatchMode,
    kinds: &[DefKind],
    results: &mut Vec<DefContent>,
    scope: &str,
) {
    if !kinds.contains(&DefKind::Concept) {
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
        kind: DefKind::Concept,
        lines: [start, end],
        signature,
        scope: build_scope(scope, "::", &name),
    });
}

fn handle_namespace_alias(
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

/// Check if a `using_declaration` node is actually a using directive
/// (`using namespace std;`), which we skip.
/// Using declarations have a `qualified_identifier` child; using directives do not.
fn is_using_directive(node: Node) -> bool {
    let mut cursor = node.walk();
    !node
        .children(&mut cursor)
        .any(|c| c.kind() == "qualified_identifier")
}

fn handle_using_declaration(
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

    // Name is in qualified_identifier > name > identifier
    let name = extract_using_decl_name(node, source);
    let name = match name {
        Some(n) => n,
        None => return,
    };
    if !mode.matches_ident(&name) {
        return;
    }

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

fn extract_using_decl_name(node: Node, source: &str) -> Option<String> {
    let mut cursor = node.walk();
    let qi = node
        .children(&mut cursor)
        .find(|c| c.kind() == "qualified_identifier")?;
    let name_node = qi.child_by_field_name("name")?;
    Some(node_text(name_node, source))
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

/// Find `operator_name` node in declarator chain (mirrors find_destructor_name).
/// Returns the operator name text (e.g., "operator+") if found.
fn find_operator_name(declarator: Node, source: &str) -> Option<String> {
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
                if inner.kind() == "operator_name" {
                    return inner
                        .utf8_text(source.as_bytes())
                        .ok()
                        .map(|s| s.to_string());
                }
                // qualified_identifier wrapping operator_name (e.g., Vec::operator+)
                if inner.kind() == "qualified_identifier" {
                    let mut cursor = inner.walk();
                    if let Some(op) = inner
                        .children(&mut cursor)
                        .find(|c| c.kind() == "operator_name")
                    {
                        return op.utf8_text(source.as_bytes()).ok().map(|s| s.to_string());
                    }
                }
                return None;
            }
            _ => return None,
        }
    }
}

/// Compute scope for anonymous struct/union/class inside a field_declaration.
/// Anonymous types have no name, so `build_scope_from_node` returns the parent scope
/// unchanged. When the anonymous type is a child of a `field_declaration` (e.g.,
/// `struct { int x; } anon;`), we use the field name as the scope parent so inner
/// fields get correct scoping like `Outer::anon::x`.
fn scope_for_anon_type_in_field(node: Node, source: &str, scope: &str) -> String {
    if node.child_by_field_name("name").is_some() {
        return build_scope_from_node(node, source, scope, "::");
    }
    if let Some(parent) = node.parent() {
        if parent.kind() == "field_declaration" {
            if let Some(declarator) = parent.child_by_field_name("declarator") {
                if let Some(name_text) = extract_field_name(declarator, source) {
                    return build_scope(scope, "::", &name_text);
                }
            }
        }
    }
    scope.to_string()
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
