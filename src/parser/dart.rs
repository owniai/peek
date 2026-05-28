use crate::model::{DefContent, DefKind};
use crate::parser::{
    LanguageParser, MatchMode, build_scope, build_scope_from_node, extract_signature_to_body,
    first_child_by_kind, first_line_of_node, line_range, node_text_ref, normalize_signature,
};
use tree_sitter::{Node, Parser};

pub(crate) const LANGUAGE: &str = "dart";
pub(crate) const EXTENSIONS: &[&str] = &["dart"];
pub(crate) const ALIASES: &[&str] = &[];

pub struct DartParser;

impl LanguageParser for DartParser {
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
            DefKind::Getter,
            DefKind::Setter,
            DefKind::Operator,
            DefKind::Class,
            DefKind::Enum,
            DefKind::Const,
            DefKind::Alias,
            DefKind::Mixin,
            DefKind::Interface,
            DefKind::Extension,
            DefKind::ExtensionType,
            DefKind::Field,
            DefKind::Module,
            DefKind::Var,
            DefKind::Variant,
        ]
    }

    impl_init_parser!(tree_sitter_dart::LANGUAGE, "Dart");

    impl_extract_with!(collect_definitions, scope: "");
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
        "library_name" => {
            handle_library(node, source, mode, kinds, results);
        }
        "class_declaration" => {
            handle_class(node, source, mode, kinds, results, scope);
        }
        "enum_declaration" => {
            handle_enum(node, source, mode, kinds, results, scope);
        }
        "mixin_declaration" => {
            handle_mixin(node, source, mode, kinds, results, scope);
        }
        "extension_declaration" => {
            handle_extension(node, source, mode, kinds, results, scope);
        }
        "extension_type_declaration" => {
            handle_extension_type(node, source, mode, kinds, results, scope);
        }
        "type_alias" => {
            handle_type_alias(node, source, mode, kinds, results, scope);
        }
        "function_signature" => {
            handle_function_sig(node, source, mode, kinds, results, scope);
        }
        "function_body" => {
            // Don't recurse into function bodies
        }
        "operator_signature" => {
            handle_operator_sig(node, source, mode, kinds, results, scope);
        }
        "factory_constructor_signature" => {
            handle_factory_constructor(node, source, mode, kinds, results, scope);
        }
        "getter_signature" | "setter_signature" => {
            handle_accessor(node, source, mode, kinds, results, scope);
        }
        "static_final_declaration_list" => {
            handle_const_list(node, source, mode, kinds, results, scope);
        }
        "enum_constant" => {
            handle_enum_constant(node, source, mode, kinds, results, scope);
        }
        "declaration" => {
            handle_declaration(node, source, mode, kinds, results, scope);
        }
        "initialized_identifier_list" if scope.is_empty() => {
            // tree-sitter-dart 0.1.0 bug: `library my_lib;` followed by other declarations
            // is misparsed as type_identifier "library" + initialized_identifier_list.
            // Detect this pattern and treat as library directive (Module) instead of Var.
            if kinds.contains(&DefKind::Module) && is_library_misparse(node, source) {
                handle_library_misparse(node, source, mode, kinds, results);
            } else if has_final_keyword(node) {
                if kinds.contains(&DefKind::Const) {
                    extract_var_names_as(node, source, mode, results, DefKind::Const);
                }
            } else if kinds.contains(&DefKind::Var) {
                extract_var_names(node, source, mode, results);
            }
        }
        "class_member" | "method_signature" => {
            recurse_children(node, source, mode, kinds, results, scope);
        }
        _ => {
            recurse_children(node, source, mode, kinds, results, scope);
        }
    }
}

/// For top-level external declarations where `external` and the signature node are siblings,
/// prepend "external " to the signature so it isn't lost.
/// Uses prev_sibling (not prev_named_sibling) because tree-sitter-dart produces
/// top-level `external` as an anonymous (unnamed) token.
fn prepend_external(node: Node, sig: &str) -> String {
    if node.prev_sibling().is_some_and(|s| s.kind() == "external") {
        format!("external {}", sig)
    } else {
        sig.to_string()
    }
}

/// Handle class_declaration: extract class definition and recurse into class_body.
/// Dart 3 `mixin class` is parsed as class_declaration with a "mixin" named child.
/// It is semantically both a class and a mixin, so we emit results for both kinds.
fn handle_class(
    node: Node,
    source: &str,
    mode: &MatchMode,
    kinds: &[DefKind],
    results: &mut Vec<DefContent>,
    scope: &str,
) {
    let own_scope = build_scope_from_node(node, source, scope, ".");
    let has_mixin = has_child_by_kind(node, "mixin");
    let has_interface = has_child_by_kind(node, "interface");

    if let Some(name_node) = node.child_by_field_name("name") {
        let name_ref = node_text_ref(name_node, source);
        if mode.matches_ident(name_ref) {
            let signature = extract_signature_to_body(node, source);
            let start_row = node.start_position().row + 1;
            let [start, end] = line_range(start_row, node);

            if kinds.contains(&DefKind::Class) {
                results.push(DefContent {
                    kind: DefKind::Class,
                    lines: [start, end],
                    signature: signature.clone(),
                    scope: own_scope.clone(),
                });
            }
            if has_mixin && kinds.contains(&DefKind::Mixin) {
                results.push(DefContent {
                    kind: DefKind::Mixin,
                    lines: [start, end],
                    signature: signature.clone(),
                    scope: own_scope.clone(),
                });
            }
            if has_interface && kinds.contains(&DefKind::Interface) {
                results.push(DefContent {
                    kind: DefKind::Interface,
                    lines: [start, end],
                    signature,
                    scope: own_scope.clone(),
                });
            }
        }
    }

    // Recurse into class_body to discover nested types
    if let Some(body) = node.child_by_field_name("body") {
        recurse_children(body, source, mode, kinds, results, &own_scope);
    }
}

/// Handle enum_declaration: extract enum definition and recurse into enum_body.
fn handle_enum(
    node: Node,
    source: &str,
    mode: &MatchMode,
    kinds: &[DefKind],
    results: &mut Vec<DefContent>,
    scope: &str,
) {
    let own_scope = build_scope_from_node(node, source, scope, ".");
    if kinds.contains(&DefKind::Enum) {
        if let Some(name_node) = node.child_by_field_name("name") {
            let name_ref = node_text_ref(name_node, source);
            if mode.matches_ident(name_ref) {
                let signature = extract_signature_to_body(node, source);
                let start_row = node.start_position().row + 1;
                let [start, end] = line_range(start_row, node);

                results.push(DefContent {
                    kind: DefKind::Enum,
                    lines: [start, end],
                    signature,
                    scope: own_scope.clone(),
                });
            }
        }
    }

    // Recurse into enum_body for nested definitions
    if let Some(body) = node.child_by_field_name("body") {
        recurse_children(body, source, mode, kinds, results, &own_scope);
    }
}

/// Handle mixin_declaration: extract mixin definition and recurse into class_body.
fn handle_mixin(
    node: Node,
    source: &str,
    mode: &MatchMode,
    kinds: &[DefKind],
    results: &mut Vec<DefContent>,
    scope: &str,
) {
    let own_scope = build_scope_from_node(node, source, scope, ".");
    if kinds.contains(&DefKind::Mixin) {
        if let Some(name_node) = first_child_by_kind(node, "identifier") {
            let name_ref = node_text_ref(name_node, source);
            if mode.matches_ident(name_ref) {
                let signature = extract_signature_to_body(node, source);
                let start_row = node.start_position().row + 1;
                let [start, end] = line_range(start_row, node);

                results.push(DefContent {
                    kind: DefKind::Mixin,
                    lines: [start, end],
                    signature,
                    scope: own_scope.clone(),
                });
            }
        }
    }

    // Recurse into class_body (mixin body is class_body)
    let mut cursor = node.walk();
    if let Some(body) = node
        .children(&mut cursor)
        .find(|c| c.kind() == "class_body")
    {
        recurse_children(body, source, mode, kinds, results, &own_scope);
    }
}

/// Handle extension_declaration and extension_type_declaration.
fn handle_extension(
    node: Node,
    source: &str,
    mode: &MatchMode,
    kinds: &[DefKind],
    results: &mut Vec<DefContent>,
    scope: &str,
) {
    let own_scope = build_scope_from_node(node, source, scope, ".");
    if kinds.contains(&DefKind::Extension) {
        let name_node = node.child_by_field_name("name");
        // extension_declaration name is optional (unnamed extensions)
        if let Some(name_node) = name_node {
            let name_ref = node_text_ref(name_node, source);
            if mode.matches_ident(name_ref) {
                let signature = extract_signature_to_body(node, source);
                let start_row = node.start_position().row + 1;
                let [start, end] = line_range(start_row, node);

                results.push(DefContent {
                    kind: DefKind::Extension,
                    lines: [start, end],
                    signature,
                    scope: own_scope.clone(),
                });
            }
        }
    }

    // Recurse into body (extension_body or class_body)
    let body = node.child_by_field_name("body");
    if let Some(body) = body {
        recurse_children(body, source, mode, kinds, results, &own_scope);
    }
}

/// Handle extension_type_declaration (Dart 3 extension types).
/// Extension types are nominal type declarations (zero-overhead type wrappers),
/// semantically distinct from behavior extensions.
fn handle_extension_type(
    node: Node,
    source: &str,
    mode: &MatchMode,
    kinds: &[DefKind],
    results: &mut Vec<DefContent>,
    scope: &str,
) {
    let own_scope = build_scope_from_node(node, source, scope, ".");
    if kinds.contains(&DefKind::ExtensionType) {
        if let Some(name_node) = node.child_by_field_name("name") {
            let name_ref = node_text_ref(name_node, source);
            if mode.matches_ident(name_ref) {
                let signature = extract_signature_to_body(node, source);
                let start_row = node.start_position().row + 1;
                let [start, end] = line_range(start_row, node);

                results.push(DefContent {
                    kind: DefKind::ExtensionType,
                    lines: [start, end],
                    signature,
                    scope: own_scope.clone(),
                });
            }
        }
    }

    // Recurse into body for methods, constructors, etc.
    let body = node.child_by_field_name("body");
    if let Some(body) = body {
        recurse_children(body, source, mode, kinds, results, &own_scope);
    }
}

/// Handle library_name: extract library directive as Module.
/// Correct AST: (library_name (dotted_identifier_list (identifier)))
fn handle_library(
    node: Node,
    source: &str,
    mode: &MatchMode,
    kinds: &[DefKind],
    results: &mut Vec<DefContent>,
) {
    if !kinds.contains(&DefKind::Module) {
        return;
    }
    if let Some(dil) = first_child_by_kind(node, "dotted_identifier_list") {
        let name = node_text_ref(dil, source);
        if mode.matches_ident(name) {
            let signature = first_line_of_node(node, source);
            let signature = normalize_signature(&signature);
            let start_row = node.start_position().row + 1;
            let [start, end] = line_range(start_row, node);
            results.push(DefContent {
                kind: DefKind::Module,
                lines: [start, end],
                signature,
                scope: name.to_string(),
            });
        }
    }
}

/// Detect tree-sitter-dart 0.1.0 misparse: `library my_lib;` followed by other
/// declarations produces type_identifier "library" + initialized_identifier_list.
fn is_library_misparse(node: Node, source: &str) -> bool {
    if let Some(prev) = node.prev_named_sibling() {
        if prev.kind() == "type_identifier" {
            return node_text_ref(prev, source) == "library";
        }
    }
    false
}

/// Handle the misparse case: extract library name from initialized_identifier_list
/// when preceded by type_identifier "library".
fn handle_library_misparse(
    node: Node,
    source: &str,
    mode: &MatchMode,
    _kinds: &[DefKind],
    results: &mut Vec<DefContent>,
) {
    if let Some(name_node) = first_child_by_kind(node, "initialized_identifier") {
        if let Some(name) = name_node.child_by_field_name("name") {
            let name_ref = node_text_ref(name, source);
            if mode.matches_ident(name_ref) {
                // Use the type_identifier + initialized_identifier_list span for line range
                let start_row = node.start_position().row + 1;
                let [start, end] = line_range(start_row, node);
                let signature = format!("library {};", name_ref);
                results.push(DefContent {
                    kind: DefKind::Module,
                    lines: [start, end],
                    signature,
                    scope: name_ref.to_string(),
                });
            }
        }
    }
}

/// Handle type_alias: extract typedef definition.
fn handle_type_alias(
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

    if let Some(name_node) = first_child_by_kind(node, "type_identifier") {
        let name_ref = node_text_ref(name_node, source);
        if mode.matches_ident(name_ref) {
            let own_scope = build_scope(scope, ".", name_ref);
            let signature = first_line_of_node(node, source);
            let signature = normalize_signature(&signature);
            let start_row = node.start_position().row + 1;
            let [start, end] = line_range(start_row, node);

            results.push(DefContent {
                kind: DefKind::Alias,
                lines: [start, end],
                signature,
                scope: own_scope,
            });
        }
    }
}

/// Handle top-level function_signature (paired with function_body as sibling).
/// In class body (scope non-empty), emits Method; at top-level (scope empty), emits Function.
fn handle_function_sig(
    node: Node,
    source: &str,
    mode: &MatchMode,
    kinds: &[DefKind],
    results: &mut Vec<DefContent>,
    scope: &str,
) {
    let def_kind = if scope.is_empty() {
        DefKind::Function
    } else {
        DefKind::Method
    };
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

    let signature = prepend_external(node, &extract_signature_to_body(node, source));
    let start_row = (node.start_position().row + 1) as u32;
    let end_row = dart_end_row(node);

    let own_scope = build_scope_from_node(node, source, scope, ".");
    results.push(DefContent {
        kind: def_kind,
        lines: [start_row, end_row],
        signature,
        scope: own_scope,
    });
}

/// Handle operator_signature (Dart operator overloading).
/// Node structure: operator_signature -> type_identifier, operator, *_operator, formal_parameter_list
fn handle_operator_sig(
    node: Node,
    source: &str,
    mode: &MatchMode,
    kinds: &[DefKind],
    results: &mut Vec<DefContent>,
    scope: &str,
) {
    if !kinds.contains(&DefKind::Operator) {
        return;
    }

    let operator_name = extract_operator_name(node, source);
    if !mode.matches_ident(&operator_name) {
        return;
    }

    let own_scope = build_scope(scope, ".", &operator_name);
    let signature = first_line_of_node(node, source);
    let signature = normalize_signature(&prepend_external(node, &signature));
    let start_row = (node.start_position().row + 1) as u32;
    let end_row = dart_end_row(node);

    results.push(DefContent {
        kind: DefKind::Operator,
        lines: [start_row, end_row],
        signature,
        scope: own_scope,
    });
}

/// Handle factory_constructor_signature node (appears inside method_signature in class body).
/// Extraction logic mirrors handle_declaration's constructor handling but with Constructor kind.
fn handle_factory_constructor(
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

    // factory_constructor_signature has name fields: class identifier, ".", constructor identifier
    let mut cursor = node.walk();
    let idents: Vec<Node> = node
        .children(&mut cursor)
        .filter(|c| c.kind() == "identifier")
        .collect();

    let (match_name, ctor_scope) = if idents.len() >= 2 {
        // Named factory constructor: Foo.admin()
        let ctor_name = node_text_ref(idents[1], source);
        (ctor_name, build_scope(scope, ".", ctor_name))
    } else if let Some(class_ident) = idents.first() {
        // Default factory constructor: Foo()
        let class_name = node_text_ref(*class_ident, source);
        (class_name, build_scope(scope, ".", class_name))
    } else {
        return;
    };

    if !mode.matches_ident(match_name) {
        return;
    }

    // Use parent node for signature if available (method_signature or class_member)
    let sig_node = node
        .parent()
        .and_then(|p| {
            if p.kind() == "method_signature" || p.kind() == "class_member" {
                Some(p)
            } else {
                None
            }
        })
        .unwrap_or(node);
    let signature = extract_signature_to_body(sig_node, source);
    let start_row = (node.start_position().row + 1) as u32;
    let end_row = dart_end_row(node);

    results.push(DefContent {
        kind: DefKind::Constructor,
        lines: [start_row, end_row],
        signature,
        scope: ctor_scope,
    });
}

/// Extract operator name from operator_signature node.
/// Finds the *_operator child (e.g., binary_operator "+") and returns "operator+".
/// Also handles anonymous operator tokens: []=[]=[]=, ~.
fn extract_operator_name(node: Node, source: &str) -> String {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.is_named() {
            if child.kind().ends_with("_operator") {
                let symbol = node_text_ref(child, source);
                return format!("operator{}", symbol);
            }
        } else if matches!(child.kind(), "[]" | "[]=" | "~") {
            return format!("operator{}", child.kind());
        }
    }
    "operator".to_string()
}

/// Handle getter_signature and setter_signature at top level or in class body.
fn handle_accessor(
    node: Node,
    source: &str,
    mode: &MatchMode,
    kinds: &[DefKind],
    results: &mut Vec<DefContent>,
    scope: &str,
) {
    let def_kind = if node.kind() == "getter_signature" {
        DefKind::Getter
    } else {
        DefKind::Setter
    };
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

    let own_scope = build_scope_from_node(node, source, scope, ".");
    let signature = first_line_of_node(node, source);
    let signature = normalize_signature(&prepend_external(node, &signature));
    let start_row = (node.start_position().row + 1) as u32;
    let end_row = dart_end_row(node);

    results.push(DefContent {
        kind: def_kind,
        lines: [start_row, end_row],
        signature,
        scope: own_scope,
    });
}

/// Handle enum_constant node inside enum_body: extract as Variant.
fn handle_enum_constant(
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
        kind: DefKind::Variant,
        lines: [start, end],
        signature,
        scope: own_scope,
    });
}

/// Handle top-level static_final_declaration_list: extract const names directly.
fn handle_const_list(
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
    extract_const_names(node, source, mode, results, scope);
}

/// Handle declaration node inside class_body or enum_body.
/// Checks children for const or constructor signatures.
fn handle_declaration(
    node: Node,
    source: &str,
    mode: &MatchMode,
    kinds: &[DefKind],
    results: &mut Vec<DefContent>,
    scope: &str,
) {
    let mut cursor = node.walk();
    let children: Vec<Node> = node.children(&mut cursor).collect();

    // Check for const (unnamed token) inside declaration
    if children
        .iter()
        .any(|c| !c.is_named() && c.kind() == "const")
    {
        if kinds.contains(&DefKind::Const) {
            if let Some(decl_list) = children
                .iter()
                .find(|c| c.kind() == "static_final_declaration_list")
            {
                extract_const_names(*decl_list, source, mode, results, scope);
            }
        }
        return;
    }

    // Check for field/var (initialized_identifier_list inside declaration, not const)
    if let Some(id_list) = children
        .iter()
        .find(|c| c.kind() == "initialized_identifier_list")
    {
        if scope.is_empty() {
            // Top-level variable → Var kind
            if kinds.contains(&DefKind::Var) {
                extract_var_names(*id_list, source, mode, results);
            }
        } else if kinds.contains(&DefKind::Field) {
            extract_field_names(*id_list, source, mode, results, scope);
        }
    }

    // Check for method signatures inside declaration (e.g., constructor)
    // For constructor_signature, we treat it as Constructor
    let callable_kinds = [
        DefKind::Constructor,
        DefKind::Method,
        DefKind::MethodDeclaration,
        DefKind::Getter,
        DefKind::Setter,
    ];
    let any_callable = callable_kinds.iter().any(|k| kinds.contains(k));
    if any_callable {
        for child in children.iter() {
            match child.kind() {
                "constructor_signature"
                | "constant_constructor_signature"
                | "factory_constructor_signature" => {
                    // constructor_signature children: identifier (class), ".", identifier (name)?
                    // Named constructor: Point.origin() → two identifiers
                    // Default constructor: Point() → one identifier (class name only)
                    let mut cursor = child.walk();
                    let idents: Vec<Node> = child
                        .children(&mut cursor)
                        .filter(|c| c.kind() == "identifier")
                        .collect();

                    let (match_name, ctor_scope) = if idents.len() >= 2 {
                        // Named constructor: match on constructor name, scope is Class.name
                        let ctor_name = node_text_ref(idents[1], source);
                        (ctor_name, build_scope(scope, ".", ctor_name))
                    } else if let Some(class_ident) = idents.first() {
                        // Default constructor: match on class name, scope is Class.ClassName
                        let class_name = node_text_ref(*class_ident, source);
                        (class_name, build_scope(scope, ".", class_name))
                    } else {
                        continue;
                    };

                    if kinds.contains(&DefKind::Constructor) && mode.matches_ident(match_name) {
                        let signature = extract_signature_to_body(node, source);
                        let start_row = (node.start_position().row + 1) as u32;
                        let end_row = dart_end_row(node);

                        results.push(DefContent {
                            kind: DefKind::Constructor,
                            lines: [start_row, end_row],
                            signature,
                            scope: ctor_scope,
                        });
                    }
                }
                "function_signature" => {
                    // function_signature inside declaration:
                    // concrete method has sibling function_body → Method
                    // abstract method has no function_body → MethodDeclaration
                    let has_body = children.iter().any(|c| c.kind() == "function_body");
                    let def_kind = if has_body {
                        DefKind::Method
                    } else {
                        DefKind::MethodDeclaration
                    };
                    if kinds.contains(&def_kind) {
                        if let Some(name_node) = child.child_by_field_name("name") {
                            let name_ref = node_text_ref(name_node, source);
                            if mode.matches_ident(name_ref) {
                                let own_scope = build_scope(scope, ".", name_ref);
                                let signature = extract_signature_to_body(node, source);
                                let start_row = (node.start_position().row + 1) as u32;
                                let end_row = dart_end_row(node);

                                results.push(DefContent {
                                    kind: def_kind,
                                    lines: [start_row, end_row],
                                    signature,
                                    scope: own_scope,
                                });
                            }
                        }
                    }
                }
                "getter_signature" | "setter_signature" => {
                    let def_kind = if child.kind() == "getter_signature" {
                        DefKind::Getter
                    } else {
                        DefKind::Setter
                    };
                    if kinds.contains(&def_kind) {
                        if let Some(name_node) = child.child_by_field_name("name") {
                            let name_ref = node_text_ref(name_node, source);
                            if mode.matches_ident(name_ref) {
                                let own_scope = build_scope(scope, ".", name_ref);
                                let signature = extract_signature_to_body(node, source);
                                let start_row = (node.start_position().row + 1) as u32;
                                let end_row = dart_end_row(node);

                                results.push(DefContent {
                                    kind: def_kind,
                                    lines: [start_row, end_row],
                                    signature,
                                    scope: own_scope,
                                });
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }
}

// === Helper functions ===

/// Check whether a node has a direct child (named or anonymous) of the given kind.
fn has_child_by_kind(node: Node, kind: &str) -> bool {
    let mut cursor = node.walk();
    node.children(&mut cursor).any(|c| c.kind() == kind)
}

/// Find the end row for a Dart definition, including the function_body if present.
/// In tree-sitter-dart, function_body is a sibling of the signature node (not a child):
/// - Top-level: function_body is a sibling of function_signature under source_file
/// - Class member: function_body is a sibling of method_signature (parent of the signature node)
fn dart_end_row(node: Node) -> u32 {
    if let Some(body) = node.next_named_sibling() {
        if body.kind() == "function_body" {
            return (body.end_position().row + 1) as u32;
        }
    }
    if let Some(parent) = node.parent() {
        if parent.kind() == "method_signature" {
            if let Some(body) = parent.next_named_sibling() {
                if body.kind() == "function_body" {
                    return (body.end_position().row + 1) as u32;
                }
            }
        }
    }
    (node.end_position().row + 1) as u32
}

/// Extract const names from a static_final_declaration_list.
/// static_final_declaration has no "name" field; the first identifier child is the name.
fn extract_const_names(
    node: Node,
    source: &str,
    mode: &MatchMode,
    results: &mut Vec<DefContent>,
    scope: &str,
) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "static_final_declaration" {
            // Find first identifier child as the name
            let mut c2 = child.walk();
            if let Some(name_node) = child.children(&mut c2).find(|c| c.kind() == "identifier") {
                let name_ref = node_text_ref(name_node, source);
                if mode.matches_ident(name_ref) {
                    let own_scope = build_scope(scope, ".", name_ref);
                    let sig = first_line_of_node(child, source);
                    let signature = normalize_signature(&sig);
                    let start_row = child.start_position().row + 1;
                    let [start, end] = line_range(start_row, child);

                    results.push(DefContent {
                        kind: DefKind::Const,
                        lines: [start, end],
                        signature,
                        scope: own_scope,
                    });
                }
            }
        }
    }
}

/// Check if the parent of `node` contains a `final` unnamed token (for `late final` detection).
fn has_final_keyword(node: Node) -> bool {
    let parent = match node.parent() {
        Some(p) => p,
        None => return false,
    };
    let mut cursor = parent.walk();
    parent
        .children(&mut cursor)
        .any(|c| !c.is_named() && c.kind() == "final")
}

/// Extract variable names from initialized_identifier_list with a configurable kind.
fn extract_var_names_as(
    node: Node,
    source: &str,
    mode: &MatchMode,
    results: &mut Vec<DefContent>,
    kind: DefKind,
) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() != "initialized_identifier" {
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

        let sig = first_line_of_node(child, source);
        let signature = normalize_signature(&sig);
        let start_row = (child.start_position().row + 1) as u32;
        let end_row = dart_end_row(child);

        results.push(DefContent {
            kind,
            lines: [start_row, end_row],
            signature,
            scope: name_ref.to_string(),
        });
    }
}

/// Extract variable names from initialized_identifier_list (Dart top-level variables).
fn extract_var_names(node: Node, source: &str, mode: &MatchMode, results: &mut Vec<DefContent>) {
    extract_var_names_as(node, source, mode, results, DefKind::Var);
}

/// Extract field names from initialized_identifier_list (Dart class fields).
fn extract_field_names(
    node: Node,
    source: &str,
    mode: &MatchMode,
    results: &mut Vec<DefContent>,
    scope: &str,
) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() != "initialized_identifier" {
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
        let sig = first_line_of_node(child, source);
        let signature = normalize_signature(&sig);
        let start_row = (child.start_position().row + 1) as u32;
        let end_row = dart_end_row(child);

        results.push(DefContent {
            kind: DefKind::Field,
            lines: [start_row, end_row],
            signature,
            scope: own_scope,
        });
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
