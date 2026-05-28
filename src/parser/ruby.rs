use crate::model::{DefContent, DefKind};
use crate::parser::{
    LanguageParser, MatchMode, build_scope, build_scope_from_node, extract_signature_to_body,
    first_line_of_node, flatten_bytes, line_range, node_text_ref, normalize_signature,
};
use tree_sitter::{Node, Parser};

pub(crate) const LANGUAGE: &str = "ruby";
pub(crate) const EXTENSIONS: &[&str] = &[
    "rb", "rake", "gemspec", "ru", "rbi", "podspec", "jbuilder", "thor", "rabl", "builder", "god",
];
pub(crate) const ALIASES: &[&str] = &["rb"];

pub struct RubyParser;

impl LanguageParser for RubyParser {
    fn language(&self) -> &'static str {
        LANGUAGE
    }

    fn extensions(&self) -> &'static [&'static str] {
        EXTENSIONS
    }

    fn supported_kinds(&self) -> &'static [DefKind] {
        &[
            DefKind::Method,
            DefKind::Constructor,
            DefKind::Class,
            DefKind::Module,
            DefKind::Struct,
            DefKind::Const,
            DefKind::Operator,
            DefKind::Var,
            DefKind::Alias,
            DefKind::Getter,
            DefKind::Setter,
            DefKind::Property,
        ]
    }

    impl_init_parser!(tree_sitter_ruby::LANGUAGE, "Ruby");

    impl_extract_with!(collect_definitions, scope: "");
}

/// Handle a container node (module or class): extract the definition and recurse into body.
fn handle_container(
    node: Node,
    source: &str,
    mode: &MatchMode,
    kinds: &[DefKind],
    results: &mut Vec<DefContent>,
    scope: &str,
    def_kind: DefKind,
) {
    let body = node.child_by_field_name("body");
    let own_scope = build_scope_from_node(node, source, scope, "::");

    if kinds.contains(&def_kind) {
        let name_node = match node.child_by_field_name("name") {
            Some(n) => n,
            None => return,
        };
        let name_ref = node_text_ref(name_node, source);

        if mode.matches_ident(name_ref) {
            let signature = extract_signature_to_body(node, source);
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

    // Always recurse into body to discover nested types
    if let Some(body) = body {
        recurse_children(body, source, mode, kinds, results, &own_scope);
    }
}

/// Handle a method node (both instance methods and singleton methods).
fn handle_method(
    node: Node,
    source: &str,
    mode: &MatchMode,
    kinds: &[DefKind],
    results: &mut Vec<DefContent>,
    scope: &str,
) {
    let name_node = match node.child_by_field_name("name") {
        Some(n) => n,
        None => return,
    };
    let name_ref = node_text_ref(name_node, source);
    let is_operator = is_ruby_operator_method(name_ref);

    let def_kind = if is_operator {
        DefKind::Operator
    } else if name_ref == "initialize" {
        DefKind::Constructor
    } else {
        DefKind::Method
    };

    if !kinds.contains(&def_kind) {
        return;
    }

    if !mode.matches_ident(name_ref) {
        return;
    }

    let own_scope = build_scope(scope, "::", name_ref);
    let signature = extract_signature_to_body(node, source);
    let start_row = node.start_position().row + 1;
    let [start, end] = line_range(start_row, node);

    results.push(DefContent {
        kind: def_kind,
        lines: [start, end],
        signature,
        scope: own_scope,
    });
}

/// Ruby operator method names that should be classified as Operator kind.
const RUBY_OPERATOR_METHODS: &[&str] = &[
    "+", "-", "*", "/", "%", "**", "==", "===", "!=", "<=>", "=~", "!~", "<", ">", "<=", ">=", "&",
    "|", "^", "~", "<<", ">>", "+@", "-@", "[]", "[]=", "`",
];

fn is_ruby_operator_method(name: &str) -> bool {
    RUBY_OPERATOR_METHODS.contains(&name)
}

/// Check if an assignment node has a constant on the left side.
fn is_constant_assignment(node: Node) -> bool {
    if let Some(left) = node.child_by_field_name("left") {
        left.kind() == "constant"
    } else {
        false
    }
}

/// Check if an assignment's RHS is a `Struct.new(...)` or `Data.define(...)` call.
fn is_struct_def_call(node: Node, source: &str) -> bool {
    let right = match node.child_by_field_name("right") {
        Some(n) if n.kind() == "call" => n,
        _ => return false,
    };
    let receiver = match right.child_by_field_name("receiver") {
        Some(n) if n.kind() == "constant" => n,
        _ => return false,
    };
    let method = match right.child_by_field_name("method") {
        Some(n) if n.kind() == "identifier" => n,
        _ => return false,
    };
    let receiver_text = node_text_ref(receiver, source);
    let method_text = node_text_ref(method, source);
    (receiver_text == "Struct" && method_text == "new")
        || (receiver_text == "Data" && method_text == "define")
}

/// Handle a Struct.new / Data.define assignment as Struct kind.
fn handle_struct_def(
    node: Node,
    source: &str,
    mode: &MatchMode,
    kinds: &[DefKind],
    results: &mut Vec<DefContent>,
    scope: &str,
) {
    if !kinds.contains(&DefKind::Struct) {
        return;
    }

    let left_node = match node.child_by_field_name("left") {
        Some(n) => n,
        None => return,
    };
    let name_ref = node_text_ref(left_node, source);

    if !mode.matches_ident(name_ref) {
        return;
    }

    let own_scope = build_scope(scope, "::", name_ref);
    let signature = first_line_of_node(node, source);
    let start_row = node.start_position().row + 1;
    let [start, end] = line_range(start_row, node);

    results.push(DefContent {
        kind: DefKind::Struct,
        lines: [start, end],
        signature,
        scope: own_scope,
    });
}

/// Handle a constant assignment node (both `assignment` and `operator_assignment`).
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

    let left_node = match node.child_by_field_name("left") {
        Some(n) => n,
        None => return,
    };
    let name_ref = node_text_ref(left_node, source);

    if !mode.matches_ident(name_ref) {
        return;
    }

    let own_scope = build_scope(scope, "::", name_ref);
    let truncation_byte = find_assignment_operator(node);

    let sig = flatten_bytes(node.start_byte(), truncation_byte, source)
        .unwrap_or_else(|| first_line_of_node(node, source));
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

/// Find the byte offset of the assignment operator for signature truncation.
/// For `operator_assignment` nodes, uses the `[operator]` field.
/// For `assignment` nodes, falls back to finding the `"="` token.
fn find_assignment_operator(node: Node) -> usize {
    if let Some(op) = node.child_by_field_name("operator") {
        return op.start_byte();
    }
    let mut cursor = node.walk();
    node.children(&mut cursor)
        .find(|c| c.kind() == "=")
        .map(|c| c.start_byte())
        .unwrap_or_else(|| node.end_byte())
}

/// Handle an `alias` node: `alias new_name old_name`.
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

    let own_scope = build_scope(scope, "::", name_ref);
    let signature = first_line_of_node(node, source);
    let start_row = node.start_position().row + 1;
    let [start, end] = line_range(start_row, node);

    results.push(DefContent {
        kind: DefKind::Alias,
        lines: [start, end],
        signature,
        scope: own_scope,
    });
}

/// Handle `alias_method :new_name, :old_name` call as Alias kind.
fn handle_alias_method(
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

    let method_node = match node.child_by_field_name("method") {
        Some(n) if n.kind() == "identifier" => n,
        _ => return,
    };
    if node_text_ref(method_node, source) != "alias_method" {
        return;
    }

    let args_node = match node.child_by_field_name("arguments") {
        Some(n) => n,
        None => return,
    };

    let name = extract_first_symbol_or_string(&args_node, source);
    let name = match name {
        Some(n) => n,
        None => return,
    };

    if !mode.matches_ident(&name) {
        return;
    }

    let own_scope = build_scope(scope, "::", &name);
    let signature = first_line_of_node(node, source);
    let start_row = node.start_position().row + 1;
    let [start, end] = line_range(start_row, node);

    results.push(DefContent {
        kind: DefKind::Alias,
        lines: [start, end],
        signature,
        scope: own_scope,
    });
}

/// Handle `attr_reader`/`attr_writer`/`attr_accessor` calls.
///
/// Each `attr_*` call produces a Property entry plus the corresponding accessor entries:
/// - `attr_reader :name`   -> Property + Getter
/// - `attr_writer :name`   -> Property + Setter
/// - `attr_accessor :name` -> Property + Getter + Setter
///
/// All entries share the same call node's line/scope/signature.
/// The output-layer dedup removes accessor entries when Property is also present.
fn handle_attr_call(
    node: Node,
    source: &str,
    mode: &MatchMode,
    kinds: &[DefKind],
    results: &mut Vec<DefContent>,
    scope: &str,
) {
    let method_node = match node.child_by_field_name("method") {
        Some(n) if n.kind() == "identifier" => n,
        _ => return,
    };
    let method_name = node_text_ref(method_node, source);
    let (want_property, want_getter, want_setter) = match method_name {
        "attr_reader" => (true, true, false),
        "attr_writer" => (true, false, true),
        "attr_accessor" => (true, true, true),
        _ => return,
    };

    let args_node = match node.child_by_field_name("arguments") {
        Some(n) => n,
        None => return,
    };

    let mut cursor = args_node.walk();
    for arg in args_node.children(&mut cursor) {
        if arg.kind() != "simple_symbol" {
            continue;
        }
        let symbol_text = node_text_ref(arg, source);
        let name = symbol_text.strip_prefix(':').unwrap_or(symbol_text);

        if !mode.matches_ident(name) {
            continue;
        }

        let own_scope = build_scope(scope, "::", name);
        let signature = first_line_of_node(node, source);
        let start_row = node.start_position().row + 1;
        let [start, end] = line_range(start_row, node);

        if want_property && kinds.contains(&DefKind::Property) {
            results.push(DefContent {
                kind: DefKind::Property,
                lines: [start, end],
                signature: signature.clone(),
                scope: own_scope.clone(),
            });
        }
        if want_getter && kinds.contains(&DefKind::Getter) {
            results.push(DefContent {
                kind: DefKind::Getter,
                lines: [start, end],
                signature: signature.clone(),
                scope: own_scope.clone(),
            });
        }
        if want_setter && kinds.contains(&DefKind::Setter) {
            results.push(DefContent {
                kind: DefKind::Setter,
                lines: [start, end],
                signature,
                scope: own_scope,
            });
        }
    }
}

/// Handle define_method / define_singleton_method metaprogramming calls.
fn handle_define_method(
    node: Node,
    source: &str,
    mode: &MatchMode,
    kinds: &[DefKind],
    results: &mut Vec<DefContent>,
    scope: &str,
) {
    let method_node = match node.child_by_field_name("method") {
        Some(n) if n.kind() == "identifier" => n,
        _ => return,
    };
    let call_name = node_text_ref(method_node, source);
    if !matches!(call_name, "define_method" | "define_singleton_method") {
        return;
    }

    let args_node = match node.child_by_field_name("arguments") {
        Some(n) => n,
        None => return,
    };

    let name = extract_first_symbol_or_string(&args_node, source);
    let name = match name {
        Some(n) => n,
        None => return,
    };

    if !kinds.contains(&DefKind::Method) || !mode.matches_ident(&name) {
        return;
    }

    let own_scope = build_scope(scope, "::", &name);
    let signature = first_line_of_node(node, source);
    let start_row = node.start_position().row + 1;
    let [start, end] = line_range(start_row, node);

    results.push(DefContent {
        kind: DefKind::Method,
        lines: [start, end],
        signature,
        scope: own_scope,
    });
}

/// Extract method name from the first simple_symbol or string argument.
fn extract_first_symbol_or_string(args_node: &Node, source: &str) -> Option<String> {
    let mut cursor = args_node.walk();
    for arg in args_node.children(&mut cursor) {
        match arg.kind() {
            "simple_symbol" => {
                let text = node_text_ref(arg, source);
                return Some(text.strip_prefix(':').unwrap_or(text).to_string());
            }
            "string" => {
                let mut sc = arg.walk();
                for child in arg.children(&mut sc) {
                    if child.kind() == "string_content" {
                        let text = node_text_ref(child, source);
                        if !text.is_empty() {
                            return Some(text.to_string());
                        }
                    }
                }
                return None;
            }
            _ => continue,
        }
    }
    None
}

/// Handle a top-level lowercase variable assignment as Var kind.
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

    let left_node = match node.child_by_field_name("left") {
        Some(n) if n.kind() == "identifier" => n,
        _ => return,
    };
    let name_ref = node_text_ref(left_node, source);

    if !mode.matches_ident(name_ref) {
        return;
    }

    let name = name_ref.to_string();
    let truncation_byte = find_assignment_operator(node);

    let sig = flatten_bytes(node.start_byte(), truncation_byte, source)
        .unwrap_or_else(|| first_line_of_node(node, source));
    let signature = normalize_signature(&sig);
    let start_row = node.start_position().row + 1;
    let [start, end] = line_range(start_row, node);

    results.push(DefContent {
        kind: DefKind::Var,
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
        "module" => {
            handle_container(node, source, mode, kinds, results, scope, DefKind::Module);
        }
        "class" => {
            handle_container(node, source, mode, kinds, results, scope, DefKind::Class);
        }
        "method" | "singleton_method" => {
            handle_method(node, source, mode, kinds, results, scope);
            // Do not recurse into method body
        }
        "alias" => {
            handle_alias(node, source, mode, kinds, results, scope);
        }
        "call" => {
            handle_attr_call(node, source, mode, kinds, results, scope);
            handle_define_method(node, source, mode, kinds, results, scope);
            handle_alias_method(node, source, mode, kinds, results, scope);
            if let Some(block) = node.child_by_field_name("block") {
                recurse_children(block, source, mode, kinds, results, scope);
            }
        }
        "assignment" | "operator_assignment" => {
            if is_constant_assignment(node) && is_struct_def_call(node, source) {
                handle_struct_def(node, source, mode, kinds, results, scope);
                // Recurse into the call's block (e.g. Struct.new(:x) do; def to_s; end; end)
                if let Some(call) = node.child_by_field_name("right") {
                    if call.kind() == "call" {
                        if let Some(block) = call.child_by_field_name("block") {
                            if let Some(left) = node.child_by_field_name("left") {
                                let name_ref = node_text_ref(left, source);
                                let struct_scope = build_scope(scope, "::", name_ref);
                                recurse_children(
                                    block,
                                    source,
                                    mode,
                                    kinds,
                                    results,
                                    &struct_scope,
                                );
                            }
                        }
                    }
                }
            } else if is_constant_assignment(node) {
                handle_const(node, source, mode, kinds, results, scope);
            } else if scope.is_empty() {
                handle_var(node, source, mode, kinds, results);
            }
        }
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
