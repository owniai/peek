use crate::model::{DefContent, DefKind};
use crate::parser::{
    LanguageParser, MatchMode, build_scope, build_scope_from_node, first_child_by_kind,
    first_line_of_node, flatten_bytes, line_range, node_text, node_text_ref, normalize_signature,
};
use tree_sitter::{Node, Parser};

pub(crate) const LANGUAGE: &str = "java";
pub(crate) const EXTENSIONS: &[&str] = &["java"];
pub(crate) const ALIASES: &[&str] = &[];

pub struct JavaParser;

impl LanguageParser for JavaParser {
    fn language(&self) -> &'static str {
        LANGUAGE
    }

    fn extensions(&self) -> &'static [&'static str] {
        EXTENSIONS
    }

    fn supported_kinds(&self) -> &'static [DefKind] {
        &[
            DefKind::Class,
            DefKind::Interface,
            DefKind::Enum,
            DefKind::Record,
            DefKind::Module,
            DefKind::Method,
            DefKind::MethodDeclaration,
            DefKind::Constructor,
            DefKind::Const,
            DefKind::Field,
            DefKind::Package,
            DefKind::Variant,
            DefKind::Annotation,
        ]
    }

    impl_init_parser!(tree_sitter_java::LANGUAGE, "Java");

    impl_extract_with!(collect_definitions, scope: "");
}

/// Common skeleton for extracting a definition from an AST node.
/// Parameterized by `def_kind` and a signature extraction strategy.
fn extract_and_push_definition(
    node: Node,
    source: &str,
    mode: &MatchMode,
    def_kind: DefKind,
    sig_extractor: impl Fn(Node, &str) -> String,
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

    let signature = sig_extractor(node, source);
    let start_row = node.start_position().row + 1;
    let [start, end] = line_range(start_row, node);

    results.push(DefContent {
        kind: def_kind,
        lines: [start, end],
        signature,
        scope: scope.to_string(),
    });
}

/// Handle a type-level definition node (class, interface, or enum).
/// Signature: up to `body` boundary, or first line as fallback.
fn handle_type_definition(
    node: Node,
    source: &str,
    mode: &MatchMode,
    def_kind: DefKind,
    results: &mut Vec<DefContent>,
    scope: &str,
) {
    extract_and_push_definition(
        node,
        source,
        mode,
        def_kind,
        |node, source| match node.child_by_field_name("body") {
            Some(body) => flatten_bytes(node.start_byte(), body.start_byte(), source)
                .unwrap_or_else(|| first_line_of_node(node, source)),
            None => first_line_of_node(node, source),
        },
        results,
        scope,
    );
}

/// Handle a callable definition node (method or constructor).
/// Signature strategy:
/// - Has `body` field: truncate to body boundary
/// - No `body` (abstract/interface method): truncate to `parameters` end boundary
/// - Fallback: first line of node
fn handle_callable(
    node: Node,
    source: &str,
    mode: &MatchMode,
    def_kind: DefKind,
    results: &mut Vec<DefContent>,
    scope: &str,
) {
    extract_and_push_definition(
        node,
        source,
        mode,
        def_kind,
        |node, source| {
            let raw_sig = if let Some(body) = node.child_by_field_name("body") {
                flatten_bytes(node.start_byte(), body.start_byte(), source)
                    .unwrap_or_else(|| first_line_of_node(node, source))
            } else if let Some(params) = node.child_by_field_name("parameters") {
                let end = if let Some(throws) = first_child_by_kind(node, "throws") {
                    throws.end_byte()
                } else {
                    params.end_byte()
                };
                flatten_bytes(node.start_byte(), end, source)
                    .unwrap_or_else(|| first_line_of_node(node, source))
            } else {
                first_line_of_node(node, source)
            };
            normalize_signature(&raw_sig)
        },
        results,
        scope,
    );
}

/// Java constants must be both static and final; fields with only one modifier are mutable state.
fn is_static_final(node: Node) -> bool {
    let modifiers = match first_child_by_kind(node, "modifiers") {
        Some(m) => m,
        None => return false,
    };
    let mut has_static = false;
    let mut has_final = false;
    let mut cursor = modifiers.walk();
    for child in modifiers.children(&mut cursor) {
        match child.kind() {
            "static" => has_static = true,
            "final" => has_final = true,
            _ => {}
        }
    }
    has_static && has_final
}

/// Handle a field_declaration node.
///
/// - Static final fields are extracted as Const kind (Java constants).
/// - All other fields are extracted as Field kind.
fn handle_field(
    node: Node,
    source: &str,
    mode: &MatchMode,
    kinds: &[DefKind],
    results: &mut Vec<DefContent>,
    scope: &str,
) {
    if is_static_final(node) {
        if !kinds.contains(&DefKind::Const) {
            return;
        }
        push_declarator_definitions(node, source, mode, results, scope, DefKind::Const);
    } else {
        if !kinds.contains(&DefKind::Field) {
            return;
        }
        push_declarator_definitions(node, source, mode, results, scope, DefKind::Field);
    }
}

/// Handle a constant_declaration node (interface constants, implicitly public static final).
fn handle_interface_const(
    node: Node,
    source: &str,
    mode: &MatchMode,
    results: &mut Vec<DefContent>,
    scope: &str,
) {
    push_declarator_definitions(node, source, mode, results, scope, DefKind::Const);
}

/// Iterate over variable_declarator children of `node`, pushing a Definition
/// for each one whose name matches `mode`.
fn push_declarator_definitions(
    node: Node,
    source: &str,
    mode: &MatchMode,
    results: &mut Vec<DefContent>,
    scope: &str,
    def_kind: DefKind,
) {
    let sig = first_line_of_node(node, source);
    let start_row = node.start_position().row + 1;
    let [start, end] = line_range(start_row, node);

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() != "variable_declarator" {
            continue;
        }
        let name_node = match child.child_by_field_name("name") {
            Some(n) => n,
            None => continue,
        };
        let name_ref = node_text_ref(name_node, source);
        if !mode.matches_ident(name_ref) {
            continue;
        }

        let own_scope = build_scope(scope, ".", name_ref);
        results.push(DefContent {
            kind: def_kind,
            lines: [start, end],
            signature: sig.clone(),
            scope: own_scope,
        });
    }
}

/// Recursively walk the AST, dispatching to type-specific handlers.
/// `scope` is the dot-separated context string (e.g. "Outer.Builder") built from
/// ancestor class/interface/enum nodes.
fn collect_definitions(
    node: Node,
    source: &str,
    mode: &MatchMode,
    kinds: &[DefKind],
    results: &mut Vec<DefContent>,
    scope: &str,
) {
    match node.kind() {
        "class_declaration" | "interface_declaration" | "record_declaration" => {
            let def_kind = match node.kind() {
                "class_declaration" => DefKind::Class,
                "interface_declaration" => DefKind::Interface,
                "record_declaration" => DefKind::Record,
                _ => unreachable!(),
            };
            let own_scope = build_scope_from_node(node, source, scope, ".");
            if kinds.contains(&def_kind) {
                handle_type_definition(node, source, mode, def_kind, results, &own_scope);
            }
            // Always recurse into body to discover nested types
            if let Some(body) = node.child_by_field_name("body") {
                recurse_children(body, source, mode, kinds, results, &own_scope);
            }
        }
        "enum_declaration" => {
            let own_scope = build_scope_from_node(node, source, scope, ".");
            if kinds.contains(&DefKind::Enum) {
                handle_type_definition(node, source, mode, DefKind::Enum, results, &own_scope);
            }
            // Build new scope so inner classes get the enum name.
            recurse_children(node, source, mode, kinds, results, &own_scope);
        }
        "enum_constant" => {
            if kinds.contains(&DefKind::Variant) {
                handle_enum_constant(node, source, mode, results, scope);
            }
            // Recurse into constant-specific class body if present
            if let Some(body) = node.child_by_field_name("body") {
                if let Some(name_node) = node.child_by_field_name("name") {
                    let name = node_text_ref(name_node, source);
                    let own_scope = build_scope(scope, ".", name);
                    recurse_children(body, source, mode, kinds, results, &own_scope);
                }
            }
        }
        "annotation_type_declaration" => {
            if kinds.contains(&DefKind::Annotation) {
                handle_annotation_type(node, source, mode, results, scope);
            }
            // Recurse into annotation_type_body for nested types and annotation elements
            if let Some(body) = node.child_by_field_name("body") {
                let own_scope = build_scope_from_node(node, source, scope, ".");
                recurse_children(body, source, mode, kinds, results, &own_scope);
            }
        }
        "annotation_type_element_declaration" => {
            if kinds.contains(&DefKind::MethodDeclaration) {
                handle_annotation_element(node, source, mode, results, scope);
            }
        }
        "method_declaration" => {
            let has_body = node.child_by_field_name("body").is_some();
            let def_kind = if has_body {
                DefKind::Method
            } else {
                DefKind::MethodDeclaration
            };
            if kinds.contains(&def_kind) {
                let callable_scope = build_scope_from_node(node, source, scope, ".");
                handle_callable(node, source, mode, def_kind, results, &callable_scope);
            }
            // Do not recurse into method body
        }
        "constructor_declaration" => {
            if kinds.contains(&DefKind::Constructor) {
                let callable_scope = build_scope_from_node(node, source, scope, ".");
                handle_callable(
                    node,
                    source,
                    mode,
                    DefKind::Constructor,
                    results,
                    &callable_scope,
                );
            }
            // Do not recurse into constructor body
        }
        "field_declaration" => {
            handle_field(node, source, mode, kinds, results, scope);
            // Do not recurse -- field initializers do not contain nested definitions
            // (anonymous class bodies in initializers are intentionally excluded)
        }
        "constant_declaration" => {
            if kinds.contains(&DefKind::Const) {
                handle_interface_const(node, source, mode, results, scope);
            }
            // Do not recurse -- interface constants do not contain nested definitions
        }
        "package_declaration" => {
            if kinds.contains(&DefKind::Package) {
                if let Some(name_node) = first_child_by_kind(node, "scoped_identifier") {
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
        "module_declaration" => {
            if kinds.contains(&DefKind::Module) {
                handle_module(node, source, mode, results);
            }
        }
        // Skip: not definitions we extract
        "import_declaration" => {}
        // Recurse into all other nodes (M2 will add specific handlers)
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

/// Handle a Java enum constant (Variant kind).
fn handle_enum_constant(
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
    let name = name_ref.to_string();
    let own_scope = build_scope(scope, ".", &name);
    let signature = first_line_of_node(node, source);
    let start_row = node.start_position().row + 1;
    let [start, end] = line_range(start_row, node);
    results.push(DefContent {
        kind: DefKind::Variant,
        lines: [start, end],
        signature,
        scope: own_scope,
    });
}

/// Handle a Java annotation type declaration (Annotation kind).
fn handle_annotation_type(
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
    let name = name_ref.to_string();
    let own_scope = build_scope(scope, ".", &name);
    let signature = first_line_of_node(node, source);
    let start_row = node.start_position().row + 1;
    let [start, end] = line_range(start_row, node);
    results.push(DefContent {
        kind: DefKind::Annotation,
        lines: [start, end],
        signature,
        scope: own_scope,
    });
}

/// Handle a Java module declaration in module-info.java (Module kind).
fn handle_module(node: Node, source: &str, mode: &MatchMode, results: &mut Vec<DefContent>) {
    let name_node = match node.child_by_field_name("name") {
        Some(n) => n,
        None => return,
    };
    let name = node_text(name_node, source);
    if !mode.matches_ident(&name) {
        return;
    }
    let signature = match node.child_by_field_name("body") {
        Some(body) => flatten_bytes(node.start_byte(), body.start_byte(), source)
            .unwrap_or_else(|| first_line_of_node(node, source)),
        None => first_line_of_node(node, source),
    };
    let start_row = node.start_position().row + 1;
    let [start, end] = line_range(start_row, node);
    results.push(DefContent {
        kind: DefKind::Module,
        lines: [start, end],
        signature,
        scope: name,
    });
}

/// Handle a Java annotation type element (MethodDeclaration kind).
/// These are the method-like declarations inside @interface bodies,
/// e.g. `String value();` or `int ttl() default 3600;`.
fn handle_annotation_element(
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
    // Signature: the full element declaration up to the semicolon
    let signature = normalize_signature(&first_line_of_node(node, source));
    let start_row = node.start_position().row + 1;
    let [start, end] = line_range(start_row, node);
    results.push(DefContent {
        kind: DefKind::MethodDeclaration,
        lines: [start, end],
        signature,
        scope: own_scope,
    });
}
