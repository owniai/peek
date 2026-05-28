use tree_sitter::Parser;

macro_rules! impl_init_parser {
    ($lang:expr, $name:expr) => {
        fn init_parser(&self) -> Parser {
            let mut parser = Parser::new();
            parser
                .set_language(&$lang.into())
                .expect(concat!($name, " language load failed"));
            parser
        }
    };
}

macro_rules! impl_extract_with {
    ($collect:ident) => {
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
            $collect(tree.root_node(), source, mode, kinds, &mut results);
            Ok(results)
        }
    };
    ($collect:ident, scope: $scope:expr) => {
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
            $collect(tree.root_node(), source, mode, kinds, &mut results, $scope);
            Ok(results)
        }
    };
    ($collect:ident, scope: $scope:expr, in_class: $in_class:expr, in_enum: $in_enum:expr) => {
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
            $collect(
                tree.root_node(),
                source,
                mode,
                kinds,
                &mut results,
                $scope,
                $in_class,
                $in_enum,
            );
            Ok(results)
        }
    };
}

pub mod bash;
pub mod c;
pub mod cpp;
pub mod csharp;
pub mod dart;
pub mod go;
pub mod java;
pub mod javascript;
pub mod kotlin;
pub mod lua;
pub mod luau;
pub mod objc;
pub mod php;
pub mod python;
pub mod ruby;
pub mod rust;
pub mod swift;
pub mod typescript;

use crate::model::{DefContent, DefKind};
pub use crate::pattern::MatchMode;

pub trait LanguageParser: Send + Sync {
    fn language(&self) -> &'static str;
    fn extensions(&self) -> &'static [&'static str];
    fn supported_kinds(&self) -> &'static [DefKind];
    fn init_parser(&self) -> Parser;
    fn extract_with(
        &self,
        mode: &MatchMode,
        kinds: &[DefKind],
        source: &str,
        parser: &mut Parser,
    ) -> Result<Vec<DefContent>, ()>;
}

use tree_sitter::Node;

pub fn first_child_by_kind<'a>(node: Node<'a>, kind: &str) -> Option<Node<'a>> {
    let mut cursor = node.walk();
    node.children(&mut cursor)
        .find(|child| child.kind() == kind)
}

pub fn first_line_of_node(node: Node, source: &str) -> String {
    let text = node.utf8_text(source.as_bytes()).unwrap_or("");
    text.lines().next().unwrap_or("").to_string()
}

pub fn node_text(node: Node, source: &str) -> String {
    node.utf8_text(source.as_bytes()).unwrap_or("").to_string()
}

pub fn node_text_ref<'a>(node: Node, source: &'a str) -> &'a str {
    node.utf8_text(source.as_bytes()).unwrap_or("")
}

/// Flatten a byte range of source into a single line by trimming each line
/// and joining with spaces.
pub fn flatten_bytes(start_byte: usize, end_byte: usize, source: &str) -> Option<String> {
    let slice = source.get(start_byte..end_byte)?;
    let parts: Vec<&str> = slice
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .collect();
    if parts.is_empty() {
        None
    } else {
        Some(parts.join(" "))
    }
}

/// Compute `[start_line, end_line]` for a definition.
pub fn line_range(start_row: usize, node: Node) -> [u32; 2] {
    [start_row as u32, (node.end_position().row + 1) as u32]
}

/// Normalize whitespace in a signature: collapse runs of whitespace to single
/// spaces and remove spaces adjacent to parentheses.
pub fn normalize_signature(sig: &str) -> String {
    let mut result = String::with_capacity(sig.len());
    let mut in_whitespace = false;
    let mut after_open_paren = false;

    for ch in sig.chars() {
        match ch {
            _ if ch.is_whitespace() => in_whitespace = true,
            '(' => {
                result.push('(');
                in_whitespace = false;
                after_open_paren = true;
            }
            ')' => {
                if result.ends_with(' ') {
                    result.pop();
                }
                result.push(')');
                in_whitespace = false;
                after_open_paren = false;
            }
            _ => {
                if in_whitespace && !result.is_empty() && !after_open_paren {
                    result.push(' ');
                }
                result.push(ch);
                in_whitespace = false;
                after_open_paren = false;
            }
        }
    }

    result
}

/// Extract function name from a declarator chain.
///
/// Recursively traverses `pointer_declarator` and `parenthesized_declarator`
/// wrappers to find a `function_declarator`, then extracts the identifier or
/// field_identifier from its `declarator` field.
///
/// Used by both C and C++ parsers to handle:
/// - Plain functions: `function_declarator` -> `identifier`
/// - Pointer-return functions: `pointer_declarator` -> `function_declarator` -> `identifier`
/// - C++ member functions: `function_declarator` -> `field_identifier`
pub fn extract_function_name(declarator: Node, source: &str) -> Option<String> {
    let mut current = declarator;
    loop {
        match current.kind() {
            "pointer_declarator" => {
                current = current.child_by_field_name("declarator")?;
            }
            "parenthesized_declarator" => {
                // parenthesized_declarator does not use field names for its child
                // nodes in tree-sitter C/C++ grammars, so we search by kind.
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
                    let last_ident = {
                        let mut cursor = inner.walk();
                        inner
                            .children(&mut cursor)
                            .filter(|c| matches!(c.kind(), "identifier" | "field_identifier"))
                            .last()
                    }?;
                    return last_ident
                        .utf8_text(source.as_bytes())
                        .ok()
                        .map(|s| s.to_string());
                }
                let text = inner.utf8_text(source.as_bytes()).ok()?;
                return Some(text.to_string());
            }
            _ => return None,
        }
    }
}

/// Check if a declaration node represents a const/constexpr variable
/// declaration.
///
/// Inspects anonymous children for `type_qualifier` nodes whose text is
/// "const" or "constexpr". Both C and C++ use the same `type_qualifier` node
/// type for these keywords (confirmed via AST experiments 2 and 4).
///
/// Returns false for function prototypes with const return types (e.g.
/// `const int compute();`, `const int *get_buf();`) where the declarator
/// chain contains a `function_declarator`.
pub fn is_const_declaration(node: Node, source: &str) -> bool {
    if let Some(mut decl) = node.child_by_field_name("declarator") {
        loop {
            match decl.kind() {
                "function_declarator" => return false,
                "pointer_declarator" | "parenthesized_declarator" => {
                    decl = match decl.child_by_field_name("declarator") {
                        Some(d) => d,
                        None => break,
                    };
                }
                _ => break,
            }
        }
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "type_qualifier" {
            if let Ok(text) = child.utf8_text(source.as_bytes()) {
                if text == "const" || text == "constexpr" {
                    return true;
                }
            }
        }
    }
    false
}

/// Extract variable name from a declarator field.
///
/// Uses the same traversal logic as `extract_const_name`:
/// unwrap `init_declarator`, then unwrap `pointer_declarator` layers,
/// then return the identifier text.
pub fn extract_var_name(declarator: Node, source: &str) -> Option<String> {
    extract_const_name(declarator, source)
}

/// Check if a declaration node has a function declarator (function prototype or definition).
///
/// Traverses through `pointer_declarator` and `parenthesized_declarator` wrappers
/// to find a `function_declarator` at any depth. Used by C/C++ parsers to distinguish
/// function declarations from variable declarations.
pub fn has_function_declarator(node: Node) -> bool {
    let Some(mut decl) = node.child_by_field_name("declarator") else {
        return false;
    };
    loop {
        match decl.kind() {
            "function_declarator" => return true,
            "pointer_declarator" | "parenthesized_declarator" => {
                decl = match decl.child_by_field_name("declarator") {
                    Some(d) => d,
                    None => return false,
                };
            }
            _ => return false,
        }
    }
}

/// Check if a declaration node has `extern` storage class specifier.
pub fn has_extern_storage_class(node: Node, source: &str) -> bool {
    let mut cursor = node.walk();
    node.children(&mut cursor).any(|c| {
        c.kind() == "storage_class_specifier" && (c.utf8_text(source.as_bytes()) == Ok("extern"))
    })
}

/// Check if a declaration node's declarator has an initializer.
pub fn has_initializer(node: Node) -> bool {
    node.child_by_field_name("declarator")
        .map(|d| d.kind() == "init_declarator")
        .unwrap_or(false)
}

/// Extract const variable name from a declarator field.
///
/// Handles two AST structures:
/// 1. With initializer: `declaration` -> `declarator` -> `init_declarator` ->
///    `declarator` -> `identifier`
///    (e.g., `const int MAX = 100`)
/// 2. Without initializer: `declaration` -> `declarator` -> `identifier`
///    (e.g., `const int MAX`)
///
/// Also handles pointer const declarations where `pointer_declarator` wraps
/// the identifier:
///    `init_declarator` -> `declarator` -> `pointer_declarator` -> `identifier`
///    (e.g., `const char *MSG = "hello"`)
pub fn extract_const_name(declarator: Node, source: &str) -> Option<String> {
    let mut current = declarator;
    // Unwrap init_declarator if present
    if current.kind() == "init_declarator" {
        current = current.child_by_field_name("declarator")?;
    }
    // Unwrap pointer_declarator layers
    while current.kind() == "pointer_declarator" {
        current = current.child_by_field_name("declarator")?;
    }
    let text = current.utf8_text(source.as_bytes()).ok()?;
    Some(text.to_string())
}

/// Extract typedef name from the declarator field of a type_definition node.
///
/// Skips function-pointer typedefs (where the declarator is a function_declarator)
/// and traverses pointer_declarator/parenthesized_declarator layers to find the
/// type_identifier. Used by both C and C++ parsers.
pub fn extract_typedef_name(declarator: Node, source: &str) -> Option<String> {
    let mut current = declarator;
    if current.kind() == "function_declarator" {
        return None;
    }
    while current.kind() == "pointer_declarator" || current.kind() == "parenthesized_declarator" {
        match current.child_by_field_name("declarator") {
            Some(inner) => current = inner,
            None => {
                let mut cursor = current.walk();
                let child = current.children(&mut cursor).find(|c| {
                    matches!(
                        c.kind(),
                        "pointer_declarator"
                            | "parenthesized_declarator"
                            | "type_identifier"
                            | "identifier"
                    )
                })?;
                current = child;
            }
        }
    }
    if current.kind() == "type_identifier" || current.kind() == "identifier" {
        Some(node_text(current, source))
    } else {
        None
    }
}

/// Extract signature from node start to the body boundary.
///
/// Used for function_definition, class_specifier, struct_specifier and other nodes
/// that have a body field. Falls back to the first line if body is absent.
pub fn extract_signature_to_body(node: Node, source: &str) -> String {
    let body = node.child_by_field_name("body");
    let end_byte = body
        .map(|b| b.start_byte())
        .unwrap_or_else(|| node.end_byte());
    let sig = flatten_bytes(node.start_byte(), end_byte, source)
        .unwrap_or_else(|| first_line_of_node(node, source));
    normalize_signature(&sig)
}

/// Classify a `method_definition` node into the appropriate callable sub-kind.
///
/// Used by both JavaScript and TypeScript parsers, which share the same
/// `method_definition` node structure in their tree-sitter grammars.
///
/// Detection priority (checked in order):
/// 1. Named `"constructor"` -> Constructor
/// 2. First unnamed child is literal `"get"` -> Getter
/// 3. First unnamed child is literal `"set"` -> Setter
/// 4. Otherwise -> Method
pub fn classify_method_definition(node: Node, source: &str) -> DefKind {
    // Check if named "constructor" (keyword)
    if let Some(name_node) = node.child_by_field_name("name") {
        if node_text_ref(name_node, source) == "constructor" {
            return DefKind::Constructor;
        }
    }

    // Scan unnamed children for "get" or "set" literal, skipping modifiers
    // (e.g. "abstract", "static", "async", "*" for generators)
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.is_named() {
            break;
        }
        if let Ok(text) = child.utf8_text(source.as_bytes()) {
            match text.trim() {
                "get" => return DefKind::Getter,
                "set" => return DefKind::Setter,
                _ => continue,
            }
        }
    }

    DefKind::Method
}

/// Return the export-aware signature start node: if `node` is wrapped in an
/// `export_statement`, return that parent; otherwise return `node` itself.
/// Used by JS and TS parsers to include the `export` keyword in signatures.
pub fn export_aware_sig_node(node: Node) -> Node {
    match node.parent() {
        Some(p) if p.kind() == "export_statement" => p,
        _ => node,
    }
}

/// Handle a `pair` node inside an object literal (e.g., `name: "hello"`,
/// `handler: function() {}`). Shared by JS and TS parsers.
///
/// - Skips `computed_property_name` keys (dynamic keys like `[expr]`).
/// - Maps function/arrow/generator values → Method, class → Class, others → Field.
/// - Signature truncates to body start for callable/class values.
pub fn handle_pair<'a>(
    node: Node<'a>,
    source: &str,
    mode: &MatchMode,
    kinds: &[DefKind],
    results: &mut Vec<DefContent>,
    scope: &str,
) {
    let key_node = match node.child_by_field_name("key") {
        Some(n) if n.kind() != "computed_property_name" => n,
        _ => return,
    };

    let name_ref = node_text_ref(key_node, source);
    if !mode.matches_ident(name_ref) {
        return;
    }

    let value_node = node.child_by_field_name("value");
    let def_kind = match value_node.as_ref().map(|v| v.kind()) {
        Some("function_expression" | "arrow_function" | "generator_function") => DefKind::Method,
        Some("class") => DefKind::Class,
        _ => DefKind::Field,
    };

    if !kinds.contains(&def_kind) {
        return;
    }

    let own_scope = build_scope(scope, ".", name_ref);
    let signature = extract_pair_signature(node, value_node, source);
    let start_row = node.start_position().row + 1;
    let [start, end] = line_range(start_row, node);

    results.push(DefContent {
        kind: def_kind,
        lines: [start, end],
        signature,
        scope: own_scope,
    });
}

/// Extract signature from a pair node, truncating to body start for
/// function/arrow/generator/class values.
fn extract_pair_signature(pair: Node, value: Option<Node>, source: &str) -> String {
    let end_byte = match value {
        Some(v) => match v.kind() {
            "function_expression" | "generator_function" => {
                first_child_by_kind(v, "statement_block")
                    .map(|b| b.start_byte())
                    .unwrap_or_else(|| pair.end_byte())
            }
            "arrow_function" => first_child_by_kind(v, "statement_block")
                .map(|b| b.start_byte())
                .unwrap_or_else(|| pair.end_byte()),
            "class" => v
                .child_by_field_name("body")
                .map(|b| b.start_byte())
                .unwrap_or_else(|| pair.end_byte()),
            _ => pair.end_byte(),
        },
        None => pair.end_byte(),
    };

    flatten_bytes(pair.start_byte(), end_byte, source)
        .unwrap_or_else(|| first_line_of_node(pair, source))
}

/// Handle `variable_declaration` node (var keyword): extract as Var kind.
/// Shared by JS and TS parsers since the AST structure is identical.
pub fn handle_var_decl<'a>(
    node: Node<'a>,
    source: &str,
    mode: &MatchMode,
    kinds: &[DefKind],
    results: &mut Vec<DefContent>,
    scope: &str,
) {
    if !kinds.contains(&DefKind::Var) {
        return;
    }

    let sig_start_node = export_aware_sig_node(node);

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

        let name = name_ref.to_string();
        let own_scope = build_scope(scope, ".", &name);
        let signature = flatten_bytes(sig_start_node.start_byte(), child.end_byte(), source)
            .unwrap_or_else(|| first_line_of_node(sig_start_node, source));
        let start_row = sig_start_node.start_position().row + 1;
        let [start, end] = line_range(start_row, node);

        results.push(DefContent {
            kind: DefKind::Var,
            lines: [start, end],
            signature,
            scope: own_scope,
        });
    }
}

/// Build a scope string by joining parent and name with a separator.
///
/// Returns `name` when parent is empty, `parent` when name is empty.
/// Used by all language parsers to construct fully qualified names (e.g. `Outer.Inner`,
/// `Module::Class`, `App\\Services\\User`).
pub fn build_scope(parent: &str, sep: &str, name: &str) -> String {
    if parent.is_empty() {
        name.to_string()
    } else if name.is_empty() {
        parent.to_string()
    } else {
        format!("{}{}{}", parent, sep, name)
    }
}

/// Build a scope string by extracting the name from a node's "name" field.
///
/// Falls back to an empty name (preserving parent scope) when the node has
/// no "name" field. Used by parsers where most declaration nodes follow the
/// standard `child_by_field_name("name")` pattern.
pub fn build_scope_from_node(node: Node, source: &str, parent: &str, sep: &str) -> String {
    let name = node
        .child_by_field_name("name")
        .map(|n| node_text(n, source))
        .unwrap_or_default();
    build_scope(parent, sep, &name)
}

/// Handle preprocessor macro definition nodes (preproc_def / preproc_function_def).
///
/// Shared by both C and C++ parsers since `#define` syntax is identical.
/// Macros don't respect C/C++ scoping rules, so scope is always the macro name itself.
pub fn handle_macro(
    node: Node,
    source: &str,
    mode: &MatchMode,
    kinds: &[DefKind],
    results: &mut Vec<DefContent>,
) {
    if !kinds.contains(&DefKind::Macro) {
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
        kind: DefKind::Macro,
        lines: [start, end],
        signature,
        scope: name,
    });
}

/// Classify a Lua/Luau metamethod name into the appropriate DefKind.
///
/// Shared by both Lua and Luau parsers since metamethod names and their
/// classifications are identical across both languages.
pub fn classify_metamethod(name: &str) -> Option<DefKind> {
    match name {
        "__add" | "__sub" | "__mul" | "__div" | "__mod" | "__pow" | "__unm" | "__concat"
        | "__eq" | "__lt" | "__le" | "__len" | "__call" | "__band" | "__bor" | "__bxor"
        | "__bnot" | "__shl" | "__shr" | "__idiv" | "__pairs" | "__ipairs" => {
            Some(DefKind::Operator)
        }
        "__index" => Some(DefKind::Getter),
        "__newindex" => Some(DefKind::Setter),
        "__gc" | "__close" => Some(DefKind::Destructor),
        "__tostring" => Some(DefKind::Operator),
        _ => None,
    }
}

/// Extract function name from a Lua/Luau name node (identifier, dot_index_expression,
/// or method_index_expression).
///
/// Returns `(full_path, final_name)` — e.g., `("app.models.create_user", "create_user")`.
/// Shared by both Lua and Luau parsers since the node types and field names are identical.
pub fn extract_dotted_name(name_node: Node, source: &str) -> Option<(String, String)> {
    match name_node.kind() {
        "identifier" => {
            let text = node_text(name_node, source);
            Some((text.clone(), text))
        }
        "dot_index_expression" | "method_index_expression" => {
            let table = name_node.child_by_field_name("table")?;
            let child = name_node
                .child_by_field_name("field")
                .or_else(|| name_node.child_by_field_name("method"))?;
            let (table_path, _) = extract_dotted_name(table, source)?;
            let child_text = node_text(child, source);
            let full_path = format!("{}.{}", table_path, child_text);
            Some((full_path, child_text))
        }
        _ => None,
    }
}
