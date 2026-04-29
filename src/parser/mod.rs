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
pub mod php;
pub mod python;
pub mod ruby;
pub mod rust;
pub mod swift;
pub mod typescript;

use crate::model::{DefContent, DefKind};
pub use crate::pattern::MatchMode;
pub use crate::pattern::ScopeFilter;

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
    fn scope_separators(&self) -> &'static [&'static str] {
        &["."]
    }
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

#[cfg(test)]
pub fn extract_definitions<P: LanguageParser>(
    parser: &P,
    name: &str,
    kinds: &[DefKind],
    source: &str,
) -> Vec<DefContent> {
    let mode = MatchMode::Exact {
        name: name.to_string(),
        case_insensitive: false,
    };
    let mut ts_parser = parser.init_parser();
    parser
        .extract_with(&mode, kinds, source, &mut ts_parser)
        .unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockParser;

    impl LanguageParser for MockParser {
        fn language(&self) -> &'static str {
            "mock"
        }
        fn extensions(&self) -> &'static [&'static str] {
            &[".mock"]
        }
        fn supported_kinds(&self) -> &'static [DefKind] {
            &[DefKind::Function]
        }
        fn init_parser(&self) -> Parser {
            Parser::new()
        }
        fn extract_with(
            &self,
            mode: &MatchMode,
            kinds: &[DefKind],
            _source: &str,
            _parser: &mut Parser,
        ) -> Result<Vec<DefContent>, ()> {
            if kinds.contains(&DefKind::Function) {
                let name = match mode {
                    MatchMode::Exact { name, .. } => name.clone(),
                    MatchMode::Fuzzy { .. } => "fuzzy".to_string(),
                    MatchMode::All => "*".to_string(),
                };
                Ok(vec![DefContent {
                    kind: DefKind::Function,
                    lines: [1, 1],
                    signature: format!("fn {}()", name),
                    scope: name,
                }])
            } else {
                Ok(vec![])
            }
        }
    }

    #[test]
    fn mock_parser_language_and_extensions() {
        let p = MockParser;
        assert_eq!(p.language(), "mock");
        assert_eq!(p.extensions(), &[".mock"]);
    }

    #[test]
    fn mock_parser_extract_filters_by_kind() {
        let p = MockParser;
        let mode = MatchMode::Exact {
            name: "test".to_string(),
            case_insensitive: false,
        };
        let mut parser = p.init_parser();
        let results = p
            .extract_with(&mode, &[DefKind::Function], "", &mut parser)
            .unwrap();
        assert_eq!(results.len(), 1);
        let mut parser = p.init_parser();
        let empty = p
            .extract_with(&mode, &[DefKind::Class], "", &mut parser)
            .unwrap();
        assert!(empty.is_empty());
    }

    // --- Helper: parse C source and return root node ---
    fn parse_c(source: &str) -> (tree_sitter::Tree, &str) {
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_c::LANGUAGE.into())
            .unwrap();
        let tree = parser.parse(source, None).unwrap();
        (tree, source)
    }

    // --- Helper: parse C++ source and return root node ---
    fn parse_cpp(source: &str) -> (tree_sitter::Tree, &str) {
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_cpp::LANGUAGE.into())
            .unwrap();
        let tree = parser.parse(source, None).unwrap();
        (tree, source)
    }

    // --- Helper: find first node of a given kind in the tree ---
    fn find_node_by_kind<'a>(node: Node<'a>, kind: &str) -> Option<Node<'a>> {
        if node.kind() == kind {
            return Some(node);
        }
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if let Some(found) = find_node_by_kind(child, kind) {
                return Some(found);
            }
        }
        None
    }

    // ============================================================
    // node_text_ref tests
    // ============================================================

    #[test]
    fn node_text_ref_matches_node_text() {
        let source = "int my_function(int a) { return a; }";
        let (tree, src) = parse_c(source);
        let func_def = find_node_by_kind(tree.root_node(), "function_definition").unwrap();
        let declarator = func_def.child_by_field_name("declarator").unwrap();
        let fd = find_node_by_kind(declarator, "identifier").unwrap();

        assert_eq!(node_text(fd, src), "my_function");
        assert_eq!(node_text_ref(fd, src), "my_function");
        // Verify node_text_ref returns &str borrowing from source
        assert!(std::ptr::eq(
            node_text_ref(fd, src).as_ptr(),
            src[node_text_ref(fd, src).as_ptr() as usize - src.as_ptr() as usize..].as_ptr()
        ));
    }

    #[test]
    fn node_text_ref_empty_for_missing_node() {
        // An identifier node with no text should return empty string
        let source = "int x;";
        let (tree, src) = parse_c(source);
        let id = find_node_by_kind(tree.root_node(), "identifier").unwrap();
        // Both functions should return "x" for a valid node
        assert_eq!(node_text(id, src), "x");
        assert_eq!(node_text_ref(id, src), "x");
    }

    // ============================================================
    // extract_function_name tests
    // ============================================================

    #[test]
    fn extract_function_name_plain_c_function() {
        let source = "int add(int a, int b) { return a + b; }";
        let (tree, src) = parse_c(source);
        let func_def = find_node_by_kind(tree.root_node(), "function_definition").unwrap();
        let declarator = func_def.child_by_field_name("declarator").unwrap();
        assert_eq!(
            super::extract_function_name(declarator, src),
            Some("add".to_string())
        );
    }

    #[test]
    fn extract_function_name_pointer_return_function() {
        let source = "char *duplicate_string(const char *src) { return ((void*)0); }";
        let (tree, src) = parse_c(source);
        let func_def = find_node_by_kind(tree.root_node(), "function_definition").unwrap();
        let declarator = func_def.child_by_field_name("declarator").unwrap();
        assert_eq!(
            super::extract_function_name(declarator, src),
            Some("duplicate_string".to_string())
        );
    }

    #[test]
    fn extract_function_name_cpp_member_function() {
        // In C++, a member function with body inside a class is a function_definition,
        // not a field_declaration.
        let source = "class Hero { int getPower() const { return power; } };";
        let (tree, src) = parse_cpp(source);
        // Find the function_definition node inside the class
        let func_def = find_node_by_kind(tree.root_node(), "function_definition").unwrap();
        let declarator = func_def.child_by_field_name("declarator").unwrap();
        assert_eq!(
            super::extract_function_name(declarator, src),
            Some("getPower".to_string())
        );
    }

    #[test]
    fn extract_function_name_plain_identifier_returns_none() {
        let source = "int add(int a, int b) { return a + b; }";
        let (tree, src) = parse_c(source);
        // Find a plain identifier node (e.g., parameter name 'a')
        let identifier = find_node_by_kind(tree.root_node(), "identifier").unwrap();
        assert_eq!(super::extract_function_name(identifier, src), None);
    }

    #[test]
    fn extract_function_name_cpp_qualified_identifier() {
        // Out-of-class method definition: void Engine::start() {}
        // The function_declarator's declarator field is a qualified_identifier
        // "Engine::start" — should return just "start" (the short name).
        let source = "void Engine::start() { }";
        let (tree, src) = parse_cpp(source);
        let func_def = find_node_by_kind(tree.root_node(), "function_definition").unwrap();
        let declarator = func_def.child_by_field_name("declarator").unwrap();
        assert_eq!(
            super::extract_function_name(declarator, src),
            Some("start".to_string())
        );
    }

    #[test]
    fn extract_function_name_parenthesized_declarator() {
        // Declaration: void ((*signal(int sig)))(int);
        // This creates nested parenthesized_declarator wrapping a pointer_declarator
        // -> function_declarator -> identifier "signal".
        // We pass the inner parenthesized_declarator to verify recursive penetration
        // through parenthesized_declarator -> pointer_declarator -> function_declarator.
        let source = "void ((*signal(int sig)))(int);";
        let (tree, src) = parse_c(source);
        // There are two parenthesized_declarator nodes; find all and use the inner one
        // that directly wraps pointer_declarator.
        let all_paren: Vec<Node> = {
            fn collect<'a>(node: Node<'a>, acc: &mut Vec<Node<'a>>) {
                if node.kind() == "parenthesized_declarator" {
                    acc.push(node);
                }
                let mut cursor = node.walk();
                for child in node.children(&mut cursor) {
                    collect(child, acc);
                }
            }
            let mut acc = Vec::new();
            collect(tree.root_node(), &mut acc);
            acc
        };
        // The innermost parenthesized_declarator wraps pointer_declarator -> function_declarator
        let inner_paren = all_paren.last().unwrap();
        assert_eq!(
            super::extract_function_name(*inner_paren, src),
            Some("signal".to_string())
        );
    }

    // ============================================================
    // is_const_declaration tests
    // ============================================================

    #[test]
    fn is_const_declaration_const_var() {
        let source = "const int MAX = 100;";
        let (tree, src) = parse_c(source);
        let decl = find_node_by_kind(tree.root_node(), "declaration").unwrap();
        assert!(super::is_const_declaration(decl, src));
    }

    #[test]
    fn is_const_declaration_constexpr_var() {
        let source = "constexpr int MAX_THREADS = 16;";
        let (tree, src) = parse_cpp(source);
        let decl = find_node_by_kind(tree.root_node(), "declaration").unwrap();
        assert!(super::is_const_declaration(decl, src));
    }

    #[test]
    fn is_const_declaration_non_const() {
        let source = "int x = 1;";
        let (tree, src) = parse_c(source);
        let decl = find_node_by_kind(tree.root_node(), "declaration").unwrap();
        assert!(!super::is_const_declaration(decl, src));
    }

    #[test]
    fn is_const_declaration_volatile_is_not_const() {
        let source = "volatile int x;";
        let (tree, src) = parse_c(source);
        let decl = find_node_by_kind(tree.root_node(), "declaration").unwrap();
        assert!(!super::is_const_declaration(decl, src));
    }

    // ============================================================
    // extract_const_name tests
    // ============================================================

    #[test]
    fn extract_const_name_with_initializer() {
        let source = "const int MAX = 100;";
        let (tree, src) = parse_c(source);
        let decl = find_node_by_kind(tree.root_node(), "declaration").unwrap();
        let declarator = decl.child_by_field_name("declarator").unwrap();
        assert_eq!(
            super::extract_const_name(declarator, src),
            Some("MAX".to_string())
        );
    }

    #[test]
    fn extract_const_name_pointer_const() {
        let source = "const char *MSG = \"hello\";";
        let (tree, src) = parse_c(source);
        let decl = find_node_by_kind(tree.root_node(), "declaration").unwrap();
        let declarator = decl.child_by_field_name("declarator").unwrap();
        assert_eq!(
            super::extract_const_name(declarator, src),
            Some("MSG".to_string())
        );
    }

    #[test]
    fn extract_const_name_without_initializer() {
        let source = "const int LIMIT;";
        let (tree, src) = parse_c(source);
        let decl = find_node_by_kind(tree.root_node(), "declaration").unwrap();
        let declarator = decl.child_by_field_name("declarator").unwrap();
        assert_eq!(
            super::extract_const_name(declarator, src),
            Some("LIMIT".to_string())
        );
    }

    #[test]
    fn extract_const_name_double_pointer() {
        let source = "const char **PTR = ((void*)0);";
        let (tree, src) = parse_c(source);
        let decl = find_node_by_kind(tree.root_node(), "declaration").unwrap();
        let declarator = decl.child_by_field_name("declarator").unwrap();
        assert_eq!(
            super::extract_const_name(declarator, src),
            Some("PTR".to_string())
        );
    }

    // ============================================================
    // normalize_signature tests
    // ============================================================

    #[test]
    fn normalize_signature_no_spaces_unchanged() {
        assert_eq!(super::normalize_signature("foo()"), "foo()");
    }

    #[test]
    fn normalize_signature_empty_parens_with_spaces() {
        assert_eq!(super::normalize_signature("foo( )"), "foo()");
    }

    #[test]
    fn normalize_signature_collapse_and_trim_parens() {
        assert_eq!(super::normalize_signature("  foo  (  x  )  "), "foo(x)");
    }

    #[test]
    fn normalize_signature_params_with_types() {
        assert_eq!(
            super::normalize_signature("pub fn foo  (  x : int  )"),
            "pub fn foo(x : int)"
        );
    }

    #[test]
    fn normalize_signature_multiple_paren_groups() {
        assert_eq!(super::normalize_signature("foo( ) ( )"), "foo()()");
    }

    #[test]
    fn normalize_signature_only_whitespace() {
        assert_eq!(super::normalize_signature("   "), "");
    }

    #[test]
    fn normalize_signature_empty() {
        assert_eq!(super::normalize_signature(""), "");
    }

    #[test]
    fn normalize_signature_nested_parens() {
        assert_eq!(super::normalize_signature("foo((x))"), "foo((x))");
    }

    // ============================================================
    // build_scope tests
    // ============================================================

    #[test]
    fn build_scope_empty_parent_returns_name() {
        assert_eq!(super::build_scope("", ".", "Foo"), "Foo");
    }

    #[test]
    fn build_scope_empty_name_returns_parent() {
        assert_eq!(super::build_scope("Parent", ".", ""), "Parent");
    }

    #[test]
    fn build_scope_both_empty_returns_empty() {
        assert_eq!(super::build_scope("", ".", ""), "");
    }

    #[test]
    fn build_scope_dot_separator() {
        assert_eq!(super::build_scope("Outer", ".", "Inner"), "Outer.Inner");
    }

    #[test]
    fn build_scope_double_colon_separator() {
        assert_eq!(super::build_scope("Outer", "::", "Inner"), "Outer::Inner");
    }

    #[test]
    fn build_scope_backslash_separator() {
        assert_eq!(
            super::build_scope("App\\Services", "\\", "User"),
            "App\\Services\\User"
        );
    }

    // ============================================================
    // build_scope_from_node tests
    // ============================================================

    #[test]
    fn build_scope_from_node_extracts_name_dot() {
        // C++ class_specifier has a "name" field pointing to type_identifier
        let source = "class MyClass { int x; };";
        let (tree, src) = parse_cpp(source);
        let class_node = find_node_by_kind(tree.root_node(), "class_specifier").unwrap();
        assert_eq!(
            super::build_scope_from_node(class_node, src, "Outer", "."),
            "Outer.MyClass"
        );
    }

    #[test]
    fn build_scope_from_node_empty_parent() {
        let source = "class MyClass { int x; };";
        let (tree, src) = parse_cpp(source);
        let class_node = find_node_by_kind(tree.root_node(), "class_specifier").unwrap();
        assert_eq!(
            super::build_scope_from_node(class_node, src, "", "."),
            "MyClass"
        );
    }

    #[test]
    fn build_scope_from_node_no_name_field_returns_parent() {
        // A C translation_unit has no "name" field → name is empty → returns parent
        let source = "int x;";
        let (tree, src) = parse_c(source);
        let root = tree.root_node();
        assert_eq!(
            super::build_scope_from_node(root, src, "Parent", "."),
            "Parent"
        );
    }
}
