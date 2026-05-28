use crate::model::{DefContent, DefKind};
use crate::parser::{
    LanguageParser, MatchMode, build_scope, first_child_by_kind, first_line_of_node, flatten_bytes,
    line_range, node_text, node_text_ref, normalize_signature,
};
use tree_sitter::{Node, Parser};

pub(crate) const LANGUAGE: &str = "swift";
pub(crate) const EXTENSIONS: &[&str] = &["swift", "swiftinterface"];
pub(crate) const ALIASES: &[&str] = &[];

pub struct SwiftParser;

impl LanguageParser for SwiftParser {
    fn language(&self) -> &'static str {
        LANGUAGE
    }

    fn extensions(&self) -> &'static [&'static str] {
        EXTENSIONS
    }

    fn supported_kinds(&self) -> &'static [DefKind] {
        &[
            DefKind::Function,
            DefKind::Method,
            DefKind::MethodDeclaration,
            DefKind::Constructor,
            DefKind::ConstructorDeclaration,
            DefKind::Class,
            DefKind::Struct,
            DefKind::Enum,
            DefKind::Protocol,
            DefKind::Alias,
            DefKind::AssociatedType,
            DefKind::Const,
            DefKind::Actor,
            DefKind::Extension,
            DefKind::Property,
            DefKind::Var,
            DefKind::Variant,
            DefKind::Destructor,
            DefKind::Subscript,
            DefKind::SubscriptDeclaration,
            DefKind::Operator,
            DefKind::OperatorDeclaration,
            DefKind::PropertyDeclaration,
            DefKind::Macro,
        ]
    }

    impl_init_parser!(tree_sitter_swift::LANGUAGE, "Swift");

    impl_extract_with!(collect_definitions, scope: "");
}

/// Determine the DefKind from a `class_declaration` node's `declaration_kind` field.
///
/// Tree-sitter-swift uses `class_declaration` for class, struct, enum, actor,
/// and extension -- distinguished by the `declaration_kind` anonymous child text.
fn classify_class_declaration(node: Node, source: &str) -> DefKind {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if !child.is_named() {
            if let Ok(text) = child.utf8_text(source.as_bytes()) {
                match text.trim() {
                    "struct" => return DefKind::Struct,
                    "enum" => return DefKind::Enum,
                    "actor" => return DefKind::Actor,
                    "extension" => return DefKind::Extension,
                    "class" => return DefKind::Class,
                    _ => continue,
                }
            }
        }
    }
    DefKind::Class
}

/// Find the body node of a class_declaration.
///
/// Body node types vary by declaration kind:
/// - class/struct/actor/extension use `class_body`
/// - enum uses `enum_class_body`
fn find_body_node(node: Node) -> Option<Node> {
    node.child_by_field_name("body").or_else(|| {
        let mut cursor = node.walk();
        node.children(&mut cursor)
            .find(|c| c.kind() == "class_body" || c.kind() == "enum_class_body")
    })
}

/// Handle a class_declaration node: extract the definition and recurse into body.
///
/// Covers Class, Struct, Enum, Actor, and Extension (after classification).
/// For extension, the name is extracted from `user_type > type_identifier`.
fn handle_class_declaration(
    node: Node,
    source: &str,
    mode: &MatchMode,
    def_kind: DefKind,
    kinds: &[DefKind],
    results: &mut Vec<DefContent>,
    scope: &str,
) {
    let body = find_body_node(node);

    let own_scope = build_scope_from_node(node, source, scope, def_kind);
    if kinds.contains(&def_kind) {
        if let Some(name_node) = node.child_by_field_name("name") {
            let name_ref = if def_kind == DefKind::Extension && name_node.kind() == "user_type" {
                first_child_by_kind(name_node, "type_identifier")
                    .or_else(|| first_child_by_kind(name_node, "simple_identifier"))
            } else {
                Some(name_node)
            };

            if let Some(name_node) = name_ref {
                let name_ref = node_text_ref(name_node, source);

                if mode.matches_ident(name_ref) {
                    let sig = match body {
                        Some(b) => flatten_bytes(node.start_byte(), b.start_byte(), source)
                            .unwrap_or_else(|| first_line_of_node(node, source)),
                        None => first_line_of_node(node, source),
                    };
                    let signature = normalize_signature(&sig);
                    let start_row = node.start_position().row + 1;
                    let [start, end] = line_range(start_row, node);

                    results.push(DefContent {
                        kind: def_kind,
                        lines: [start, end],
                        signature,
                        scope: own_scope.clone(),
                    });
                }
            }
        }
    }

    // Always recurse into body to discover nested types
    if let Some(body) = body {
        recurse_children(body, source, mode, kinds, results, &own_scope);
    }
}

/// Extract the name text from a class_declaration node.
///
/// For extension, name is in `user_type > type_identifier` (accessed via the `name` field).
/// For other types, name is a direct `type_identifier` via the `name` field.
fn extract_declaration_name(node: Node, source: &str, def_kind: DefKind) -> Option<String> {
    let name_node = node.child_by_field_name("name")?;
    if def_kind == DefKind::Extension {
        // Extension name is `user_type(type_identifier)` -- get the inner type_identifier
        if name_node.kind() == "user_type" {
            let inner = first_child_by_kind(name_node, "type_identifier")
                .or_else(|| first_child_by_kind(name_node, "simple_identifier"));
            return inner.map(|n| node_text(n, source));
        }
    }
    Some(node_text(name_node, source))
}

/// Handle a protocol_declaration node.
fn handle_protocol(
    node: Node,
    source: &str,
    mode: &MatchMode,
    kinds: &[DefKind],
    results: &mut Vec<DefContent>,
    scope: &str,
) {
    let body = first_child_by_kind(node, "protocol_body");
    let own_scope = build_scope_from_node(node, source, scope, DefKind::Protocol);

    if kinds.contains(&DefKind::Protocol) {
        let name_node = match node.child_by_field_name("name") {
            Some(n) => n,
            None => return,
        };
        let name_ref = node_text_ref(name_node, source);

        if mode.matches_ident(name_ref) {
            let sig = match body {
                Some(b) => flatten_bytes(node.start_byte(), b.start_byte(), source)
                    .unwrap_or_else(|| first_line_of_node(node, source)),
                None => first_line_of_node(node, source),
            };
            let signature = normalize_signature(&sig);
            let start_row = node.start_position().row + 1;
            let [start, end] = line_range(start_row, node);

            results.push(DefContent {
                kind: DefKind::Protocol,
                lines: [start, end],
                signature,
                scope: own_scope.clone(),
            });
        }
    }

    // Always recurse into body
    if let Some(body) = body {
        recurse_children(body, source, mode, kinds, results, &own_scope);
    }
}

/// Handle a function_declaration or protocol_function_declaration node.
fn handle_function(
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

    let name_node = match node.child_by_field_name("name") {
        Some(n) => n,
        None => return,
    };
    let name_ref = node_text_ref(name_node, source);

    if !mode.matches_ident(name_ref) {
        return;
    }

    let own_scope = build_scope(scope, ".", name_ref);
    let raw_sig = if let Some(body) = first_child_by_kind(node, "function_body") {
        flatten_bytes(node.start_byte(), body.start_byte(), source)
            .unwrap_or_else(|| first_line_of_node(node, source))
    } else {
        // Protocol function declaration -- no body
        first_line_of_node(node, source)
    };
    let signature = normalize_signature(&raw_sig);
    let start_row = node.start_position().row + 1;
    let [start, end] = line_range(start_row, node);

    results.push(DefContent {
        kind: def_kind,
        lines: [start, end],
        signature,
        scope: own_scope,
    });
}

/// Handle an init_declaration node (Swift initializer / constructor).
fn handle_init(
    node: Node,
    source: &str,
    mode: &MatchMode,
    kinds: &[DefKind],
    results: &mut Vec<DefContent>,
    scope: &str,
) {
    if !kinds.contains(&DefKind::Constructor) {
        return;
    }

    // init has no name field -- use "init" as the name (or scope prefix for qualified init)
    let name = "init";
    if !mode.matches_ident(name) {
        return;
    }

    let own_scope = build_scope(scope, ".", name);
    let raw_sig = if let Some(body) = first_child_by_kind(node, "function_body") {
        flatten_bytes(node.start_byte(), body.start_byte(), source)
            .unwrap_or_else(|| first_line_of_node(node, source))
    } else {
        first_line_of_node(node, source)
    };
    let signature = normalize_signature(&raw_sig);
    let start_row = node.start_position().row + 1;
    let [start, end] = line_range(start_row, node);

    results.push(DefContent {
        kind: DefKind::Constructor,
        lines: [start, end],
        signature,
        scope: own_scope,
    });
}

/// Handle a deinit_declaration node (Swift deinitializer / destructor).
fn handle_deinit(
    node: Node,
    source: &str,
    mode: &MatchMode,
    kinds: &[DefKind],
    results: &mut Vec<DefContent>,
    scope: &str,
) {
    if !kinds.contains(&DefKind::Destructor) {
        return;
    }

    let name = "deinit";
    if !mode.matches_ident(name) {
        return;
    }

    let own_scope = build_scope(scope, ".", name);
    let raw_sig = if let Some(body) = first_child_by_kind(node, "function_body") {
        flatten_bytes(node.start_byte(), body.start_byte(), source)
            .unwrap_or_else(|| first_line_of_node(node, source))
    } else {
        first_line_of_node(node, source)
    };
    let signature = normalize_signature(&raw_sig);
    let start_row = node.start_position().row + 1;
    let [start, end] = line_range(start_row, node);

    results.push(DefContent {
        kind: DefKind::Destructor,
        lines: [start, end],
        signature,
        scope: own_scope,
    });
}

/// Handle a subscript_declaration node (Swift subscript).
/// Subscripts have no name field, so we use "subscript" as the identifier.
fn handle_subscript(
    node: Node,
    source: &str,
    mode: &MatchMode,
    kinds: &[DefKind],
    results: &mut Vec<DefContent>,
    scope: &str,
) {
    if !kinds.contains(&DefKind::Subscript) {
        return;
    }

    let name = "subscript";
    if !mode.matches_ident(name) {
        return;
    }

    let own_scope = build_scope(scope, ".", name);
    let raw_sig = if let Some(body) = first_child_by_kind(node, "computed_property") {
        flatten_bytes(node.start_byte(), body.start_byte(), source)
            .unwrap_or_else(|| first_line_of_node(node, source))
    } else {
        first_line_of_node(node, source)
    };
    let signature = normalize_signature(&raw_sig);
    let start_row = node.start_position().row + 1;
    let [start, end] = line_range(start_row, node);

    results.push(DefContent {
        kind: DefKind::Subscript,
        lines: [start, end],
        signature,
        scope: own_scope,
    });
}

/// Handle an init_declaration node that is a protocol requirement (no body).
/// Maps to ConstructorDeclaration instead of Constructor.
fn handle_init_declaration(
    node: Node,
    source: &str,
    mode: &MatchMode,
    kinds: &[DefKind],
    results: &mut Vec<DefContent>,
    scope: &str,
) {
    if !kinds.contains(&DefKind::ConstructorDeclaration) {
        return;
    }

    let name = "init";
    if !mode.matches_ident(name) {
        return;
    }

    let own_scope = build_scope(scope, ".", name);
    let sig = first_line_of_node(node, source);
    let signature = normalize_signature(&sig);
    let start_row = node.start_position().row + 1;
    let [start, end] = line_range(start_row, node);

    results.push(DefContent {
        kind: DefKind::ConstructorDeclaration,
        lines: [start, end],
        signature,
        scope: own_scope,
    });
}

/// Handle a subscript_declaration node that is a protocol requirement (no concrete body).
/// Maps to SubscriptDeclaration instead of Subscript.
fn handle_subscript_declaration(
    node: Node,
    source: &str,
    mode: &MatchMode,
    kinds: &[DefKind],
    results: &mut Vec<DefContent>,
    scope: &str,
) {
    if !kinds.contains(&DefKind::SubscriptDeclaration) {
        return;
    }

    let name = "subscript";
    if !mode.matches_ident(name) {
        return;
    }

    let own_scope = build_scope(scope, ".", name);
    let raw_sig = if let Some(cp) = first_child_by_kind(node, "computed_property") {
        flatten_bytes(node.start_byte(), cp.start_byte(), source)
            .unwrap_or_else(|| first_line_of_node(node, source))
    } else {
        first_line_of_node(node, source)
    };
    let signature = normalize_signature(&raw_sig);
    let start_row = node.start_position().row + 1;
    let [start, end] = line_range(start_row, node);

    results.push(DefContent {
        kind: DefKind::SubscriptDeclaration,
        lines: [start, end],
        signature,
        scope: own_scope,
    });
}
///
/// Tree-sitter-swift uses `enum_entry` nodes inside `enum_class_body`.
/// Each `enum_entry` has one or more `name` fields (simple_identifier).
/// Multiple cases on one line produce multiple `name` children.
fn handle_enum_entry(
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

    // Collect all name fields — an enum_entry can declare multiple variants
    let mut names: Vec<String> = Vec::new();
    let mut cursor = node.walk();
    for child in node.children_by_field_name("name", &mut cursor) {
        let name_ref = node_text_ref(child, source);
        if mode.matches_ident(name_ref) {
            names.push(name_ref.to_string());
        }
    }

    let start_row = node.start_position().row + 1;
    let [start, end] = line_range(start_row, node);
    let sig = first_line_of_node(node, source);
    let signature = normalize_signature(&sig);

    for name in names {
        let own_scope = build_scope(scope, ".", &name);

        results.push(DefContent {
            kind: DefKind::Variant,
            lines: [start, end],
            signature: signature.clone(),
            scope: own_scope,
        });
    }
}

/// Handle an associatedtype_declaration node (Swift protocol associated type).
///
/// Maps to DefKind::AssociatedType (protocol abstract type requirement).
fn handle_associatedtype(
    node: Node,
    source: &str,
    mode: &MatchMode,
    kinds: &[DefKind],
    results: &mut Vec<DefContent>,
    scope: &str,
) {
    if !kinds.contains(&DefKind::AssociatedType) {
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

    let own_scope = build_scope(scope, ".", name_ref);
    let sig = first_line_of_node(node, source);
    let signature = normalize_signature(&sig);
    let start_row = node.start_position().row + 1;
    let [start, end] = line_range(start_row, node);

    results.push(DefContent {
        kind: DefKind::AssociatedType,
        lines: [start, end],
        signature,
        scope: own_scope,
    });
}

/// Handle a typealias_declaration node.
fn handle_typealias(
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

    let own_scope = build_scope(scope, ".", name_ref);
    let sig = first_line_of_node(node, source);
    let signature = normalize_signature(&sig);
    let start_row = node.start_position().row + 1;
    let [start, end] = line_range(start_row, node);

    results.push(DefContent {
        kind: DefKind::Alias,
        lines: [start, end],
        signature,
        scope: own_scope,
    });
}

/// Handle a property_declaration node, extracting only `let` constants.
///
/// Tree-sitter-swift property_declaration structure:
/// - `value_binding_pattern` with `mutability` field ("let" or "var")
/// - `pattern` with `bound_identifier` field containing `simple_identifier`
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

    // Check if this is a let (constant) declaration
    if !is_let_property(node, source) {
        return;
    }

    // Extract name from pattern > bound_identifier > simple_identifier
    let name_ref = extract_property_name_node(node).map(|n| node_text_ref(n, source));

    if let Some(name_ref) = name_ref {
        if !mode.matches_ident(name_ref) {
            return;
        }

        let name = name_ref.to_string();
        let own_scope = if scope.is_empty() {
            name
        } else {
            format!("{}.{}", scope, name)
        };
        let sig = first_line_of_node(node, source);
        let signature = normalize_signature(&sig);
        let start_row = node.start_position().row + 1;
        let [start, end] = line_range(start_row, node);

        results.push(DefContent {
            kind: DefKind::Const,
            lines: [start, end],
            signature,
            scope: own_scope,
        });
    }
}

/// Check if a function_declaration has an operator symbol as its name.
/// Swift operator characters: / = - + ! * % < > & | ^ ~ ?
fn is_swift_operator_function(node: Node, source: &str) -> bool {
    if let Some(name_node) = node.child_by_field_name("name") {
        let name = node_text_ref(name_node, source);
        return is_swift_operator_name(name);
    }
    false
}

fn is_swift_operator_name(name: &str) -> bool {
    !name.is_empty()
        && name.chars().all(|c| {
            matches!(
                c,
                '/' | '=' | '-' | '+' | '!' | '*' | '%' | '<' | '>' | '&' | '|' | '^' | '~' | '?'
            )
        })
}

/// Check if a property_declaration uses `let` (constant) binding.
fn is_let_property(node: Node, source: &str) -> bool {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "value_binding_pattern" {
            if let Some(mut_field) = child.child_by_field_name("mutability") {
                if let Ok(text) = mut_field.utf8_text(source.as_bytes()) {
                    return text.trim() == "let";
                }
            }
        }
    }
    false
}

/// Extract property name node (bound_identifier) from a property_declaration node.
fn extract_property_name_node(node: Node) -> Option<Node> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "pattern" {
            if let Some(bound_id) = child.child_by_field_name("bound_identifier") {
                return Some(bound_id);
            }
        }
    }
    None
}

/// Handle a property_declaration with `var` binding as Property kind.
fn handle_property(
    node: Node,
    source: &str,
    mode: &MatchMode,
    results: &mut Vec<DefContent>,
    scope: &str,
) {
    let name_ref = extract_property_name_node(node).map(|n| node_text_ref(n, source));

    if let Some(name_ref) = name_ref {
        if !mode.matches_ident(name_ref) {
            return;
        }

        let own_scope = build_scope(scope, ".", name_ref);
        let sig = first_line_of_node(node, source);
        let signature = normalize_signature(&sig);
        let start_row = node.start_position().row + 1;
        let [start, end] = line_range(start_row, node);

        results.push(DefContent {
            kind: DefKind::Property,
            lines: [start, end],
            signature,
            scope: own_scope,
        });
    }
}

/// Handle a property_declaration with `var` binding at top level as Var kind.
fn handle_var(
    node: Node,
    source: &str,
    mode: &MatchMode,
    kinds: &[DefKind],
    results: &mut Vec<DefContent>,
) {
    if !kinds.contains(&DefKind::Var) {
        return;
    }

    let name_ref = extract_property_name_node(node).map(|n| node_text_ref(n, source));

    if let Some(name_ref) = name_ref {
        if !mode.matches_ident(name_ref) {
            return;
        }

        let sig = first_line_of_node(node, source);
        let signature = normalize_signature(&sig);
        let start_row = node.start_position().row + 1;
        let [start, end] = line_range(start_row, node);

        results.push(DefContent {
            kind: DefKind::Var,
            lines: [start, end],
            signature,
            scope: name_ref.to_string(),
        });
    }
}

/// Extract the macro name from a `macro_declaration` node.
///
/// `macro_declaration` has no `name` field — the name is the first `simple_identifier`
/// child after the `macro` keyword token.
fn extract_macro_name<'a>(node: Node<'a>, source: &'a str) -> Option<&'a str> {
    let mut found_macro_kw = false;
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if !found_macro_kw {
            if !child.is_named() {
                if let Ok(text) = child.utf8_text(source.as_bytes()) {
                    if text.trim() == "macro" {
                        found_macro_kw = true;
                    }
                }
            }
            continue;
        }
        if child.kind() == "simple_identifier" {
            return node_text_ref(child, source).into();
        }
    }
    None
}

/// Handle a macro_declaration node (Swift 5.9+ macro definition).
fn handle_macro(
    node: Node,
    source: &str,
    mode: &MatchMode,
    kinds: &[DefKind],
    results: &mut Vec<DefContent>,
    scope: &str,
) {
    if !kinds.contains(&DefKind::Macro) {
        return;
    }

    let name_ref = match extract_macro_name(node, source) {
        Some(n) => n,
        None => return,
    };

    if !mode.matches_ident(name_ref) {
        return;
    }

    let own_scope = build_scope(scope, ".", name_ref);
    let raw_sig = if let Some(def_node) = node.child_by_field_name("definition") {
        flatten_bytes(node.start_byte(), def_node.start_byte(), source)
            .unwrap_or_else(|| first_line_of_node(node, source))
    } else {
        first_line_of_node(node, source)
    };
    let signature = normalize_signature(&raw_sig);
    let start_row = node.start_position().row + 1;
    let [start, end] = line_range(start_row, node);

    results.push(DefContent {
        kind: DefKind::Macro,
        lines: [start, end],
        signature,
        scope: own_scope,
    });
}

/// Handle an operator_declaration node (e.g. `infix operator +++: AdditionPrecedence`).
///
/// The operator name is in a `custom_operator` child node.
fn handle_operator_declaration(
    node: Node,
    source: &str,
    mode: &MatchMode,
    kinds: &[DefKind],
    results: &mut Vec<DefContent>,
    scope: &str,
) {
    if !kinds.contains(&DefKind::OperatorDeclaration) {
        return;
    }

    let name_ref = {
        let mut cursor = node.walk();
        node.children(&mut cursor)
            .find(|c| c.kind() == "custom_operator")
            .map(|c| node_text_ref(c, source))
    };

    if let Some(name_ref) = name_ref {
        if !mode.matches_ident(name_ref) {
            return;
        }

        let own_scope = build_scope(scope, ".", name_ref);
        let sig = first_line_of_node(node, source);
        let signature = normalize_signature(&sig);
        let start_row = node.start_position().row + 1;
        let [start, end] = line_range(start_row, node);

        results.push(DefContent {
            kind: DefKind::OperatorDeclaration,
            lines: [start, end],
            signature,
            scope: own_scope,
        });
    }
}

fn handle_precedence_group(
    node: Node,
    source: &str,
    mode: &MatchMode,
    kinds: &[DefKind],
    results: &mut Vec<DefContent>,
) {
    if !kinds.contains(&DefKind::Operator) {
        return;
    }

    let name_node = first_child_by_kind(node, "simple_identifier");
    let Some(name_node) = name_node else { return };

    let name = node_text(name_node, source);
    if !mode.matches_ident(&name) {
        return;
    }

    let sig = first_line_of_node(node, source);
    let signature = normalize_signature(&sig);
    let start_row = node.start_position().row + 1;
    let [start, end] = line_range(start_row, node);

    results.push(DefContent {
        kind: DefKind::Operator,
        lines: [start, end],
        signature,
        scope: name,
    });
}

/// Recursively walk the AST, dispatching to type-specific handlers.
fn collect_definitions(
    node: Node,
    source: &str,
    mode: &MatchMode,
    kinds: &[DefKind],
    results: &mut Vec<DefContent>,
    scope: &str,
) {
    match node.kind() {
        "class_declaration" => {
            let def_kind = classify_class_declaration(node, source);
            handle_class_declaration(node, source, mode, def_kind, kinds, results, scope);
        }
        "protocol_declaration" => {
            handle_protocol(node, source, mode, kinds, results, scope);
        }
        "function_declaration" => {
            let def_kind = if is_swift_operator_function(node, source) {
                DefKind::Operator
            } else if scope.is_empty() {
                DefKind::Function
            } else {
                DefKind::Method
            };
            handle_function(node, source, mode, kinds, results, scope, def_kind);
        }
        "protocol_function_declaration" => {
            // Protocol function declarations have no body -- they are declarations, not definitions.
            // Map to declaration variants: Method → MethodDeclaration, Operator → OperatorDeclaration.
            let def_kind = if is_swift_operator_function(node, source) {
                DefKind::Operator
                    .declaration_pair()
                    .unwrap_or(DefKind::OperatorDeclaration)
            } else {
                DefKind::Method
                    .declaration_pair()
                    .unwrap_or(DefKind::MethodDeclaration)
            };
            handle_function(node, source, mode, kinds, results, scope, def_kind);
        }
        "init_declaration" => {
            // Protocol init declarations have no function_body; concrete inits have one.
            let has_body = first_child_by_kind(node, "function_body").is_some();
            if has_body {
                handle_init(node, source, mode, kinds, results, scope);
            } else {
                handle_init_declaration(node, source, mode, kinds, results, scope);
            }
        }
        "deinit_declaration" => {
            handle_deinit(node, source, mode, kinds, results, scope);
        }
        "subscript_declaration" => {
            // Concrete subscripts have a computed_property (getter/setter blocks) or body with function_body;
            // protocol subscript requirements are inside protocol_body and have computed_property
            // with only getter_specifier/setter_specifier (no implementation).
            let in_protocol = node
                .parent()
                .map(|p| p.kind() == "protocol_body")
                .unwrap_or(false);
            let has_concrete_body = !in_protocol
                && (first_child_by_kind(node, "computed_property").is_some()
                    || node
                        .child_by_field_name("body")
                        .map(|b| first_child_by_kind(b, "function_body").is_some())
                        .unwrap_or(false));
            if has_concrete_body {
                handle_subscript(node, source, mode, kinds, results, scope);
            } else {
                handle_subscript_declaration(node, source, mode, kinds, results, scope);
            }
        }
        "enum_entry" => {
            handle_enum_entry(node, source, mode, kinds, results, scope);
        }
        "associatedtype_declaration" => {
            handle_associatedtype(node, source, mode, kinds, results, scope);
        }
        "typealias_declaration" => {
            handle_typealias(node, source, mode, kinds, results, scope);
        }
        "macro_declaration" => {
            handle_macro(node, source, mode, kinds, results, scope);
        }
        "operator_declaration" => {
            handle_operator_declaration(node, source, mode, kinds, results, scope);
        }
        "precedence_group_declaration" => {
            handle_precedence_group(node, source, mode, kinds, results);
        }
        "property_declaration" => {
            if is_let_property(node, source) {
                handle_const(node, source, mode, kinds, results, scope);
            } else if scope.is_empty() {
                // Top-level var → Var kind
                handle_var(node, source, mode, kinds, results);
            } else if kinds.contains(&DefKind::Property) {
                handle_property(node, source, mode, results, scope);
            }
        }
        "protocol_property_declaration" => {
            if !kinds.contains(&DefKind::PropertyDeclaration) {
                return;
            }
            if is_let_property(node, source) {
                return;
            }
            let name_ref = extract_property_name_node(node).map(|n| node_text_ref(n, source));
            if let Some(name_ref) = name_ref {
                if !mode.matches_ident(name_ref) {
                    return;
                }
                let own_scope = build_scope(scope, ".", name_ref);
                let sig = first_line_of_node(node, source);
                let signature = normalize_signature(&sig);
                let start_row = node.start_position().row + 1;
                let [start, end] = line_range(start_row, node);
                results.push(DefContent {
                    kind: DefKind::PropertyDeclaration,
                    lines: [start, end],
                    signature,
                    scope: own_scope,
                });
            }
        }
        _ => {
            recurse_children(node, source, mode, kinds, results, scope);
        }
    }
}

/// Build a new scope string by appending the current node's name to the parent scope.
fn build_scope_from_node(
    node: Node,
    source: &str,
    parent_scope: &str,
    def_kind: DefKind,
) -> String {
    let name = extract_declaration_name(node, source, def_kind).unwrap_or_default();
    build_scope(parent_scope, ".", &name)
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
