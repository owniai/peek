use crate::model::{DefContent, DefKind};
use crate::parser::{
    LanguageParser, MatchMode, build_scope, first_child_by_kind, first_line_of_node, flatten_bytes,
    line_range, node_text, node_text_ref,
};
use tree_sitter::{Node, Parser};

pub struct PythonParser;

impl LanguageParser for PythonParser {
    fn language(&self) -> &'static str {
        "py"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &[".py", ".pyw", ".pyi", ".gyp", ".gypi", ".wsgi"]
    }

    fn supported_kinds(&self) -> &'static [DefKind] {
        &[
            DefKind::Function,
            DefKind::Method,
            DefKind::Class,
            DefKind::Alias,
        ]
    }

    impl_init_parser!(tree_sitter_python::LANGUAGE, "Python");

    impl_extract_with!(collect_definitions, scope: "");
}

fn kind_for_node(node: Node) -> Option<DefKind> {
    match node.kind() {
        "function_definition" => Some(DefKind::Function),
        "class_definition" => Some(DefKind::Class),
        "type_alias_statement" => Some(DefKind::Alias),
        _ => None,
    }
}

/// Extract name from `type_alias_statement` node.
/// PEP 695: `type Foo = ...` or `type Foo[T] = ...`
/// Structure: `left` field is a `type` node containing `identifier` (or `generic_type > identifier`).
fn extract_type_alias_name(node: Node, source: &str) -> Option<String> {
    let left = node.child_by_field_name("left")?;
    let mut cursor = left.walk();
    for child in left.children(&mut cursor) {
        match child.kind() {
            "identifier" => return Some(node_text(child, source)),
            "generic_type" => {
                if let Some(id) = first_child_by_kind(child, "identifier") {
                    return Some(node_text(id, source));
                }
            }
            _ => {}
        }
    }
    None
}

/// Check if an `assignment` node is a `TypeAlias` annotated type alias
/// (e.g. `HeaderValue: TypeAlias = str | list[str]`).
/// Returns the name if the `type:` annotation is `TypeAlias` (or `typing.TypeAlias`).
fn try_extract_typealias_name(node: Node, source: &str) -> Option<String> {
    if node.kind() != "assignment" {
        return None;
    }
    let type_node = node.child_by_field_name("type")?;
    if !is_typealias_annotation(&type_node, source) {
        return None;
    }
    let left = node.child_by_field_name("left")?;
    if left.kind() == "identifier" {
        return Some(node_text(left, source));
    }
    None
}

fn is_typealias_annotation(type_node: &Node, source: &str) -> bool {
    let mut cursor = type_node.walk();
    for child in type_node.children(&mut cursor) {
        match child.kind() {
            "identifier" if node_text_ref(child, source) == "TypeAlias" => {
                return true;
            }
            "attribute" => {
                if let Some(attr) = child.child_by_field_name("attribute") {
                    if node_text_ref(attr, source) == "TypeAlias" {
                        return true;
                    }
                }
            }
            _ => {}
        }
    }
    false
}

fn collect_definitions<'a>(
    node: Node<'a>,
    source: &str,
    mode: &MatchMode,
    kinds: &[DefKind],
    results: &mut Vec<DefContent>,
    scope: &str,
) {
    let kind = node.kind();

    if kind == "decorated_definition" {
        if let Some(inner) = first_child_by_kind(node, "function_definition") {
            try_add_definition(inner, source, mode, kinds, results, scope, Some(node));
        }
        if let Some(inner) = first_child_by_kind(node, "class_definition") {
            try_add_definition(inner, source, mode, kinds, results, scope, Some(node));
        }
        return;
    }

    try_add_definition(node, source, mode, kinds, results, scope, None);
}

fn try_add_definition<'a>(
    node: Node<'a>,
    source: &str,
    mode: &MatchMode,
    kinds: &[DefKind],
    results: &mut Vec<DefContent>,
    scope: &str,
    outer: Option<Node<'a>>,
) {
    let kind = node.kind();

    if kind == "function_definition" || kind == "class_definition" || kind == "type_alias_statement"
    {
        let def_kind = kind_for_node(node).unwrap();

        if kind == "type_alias_statement" {
            let name_text = extract_type_alias_name(node, source);
            let own_scope = match &name_text {
                Some(name) => build_scope(scope, ".", name),
                None => scope.to_string(),
            };
            if kinds.contains(&def_kind) {
                if let Some(ident_text) = &name_text {
                    if mode.matches_ident(ident_text) {
                        let start_row = outer
                            .map(|n| n.start_position().row + 1)
                            .unwrap_or_else(|| node.start_position().row + 1);
                        let start_byte = outer
                            .map(|n| n.start_byte())
                            .unwrap_or_else(|| node.start_byte());
                        let end_byte = match first_child_by_kind(node, "block") {
                            Some(body) => body.start_byte(),
                            None => node.end_byte(),
                        };
                        let raw = flatten_bytes(start_byte, end_byte, source)
                            .unwrap_or_else(|| first_line_of_node(node, source));
                        let signature = clean_signature(&raw);
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
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                collect_definitions(child, source, mode, kinds, results, &own_scope);
            }
            return;
        }

        // function_definition / class_definition
        let mut def_kind = kind_for_node(node).unwrap();

        // In Python, scope is only set by class_definition, so !scope.is_empty()
        // means this function_definition is inside a class body → Method
        if kind == "function_definition" && !scope.is_empty() {
            def_kind = DefKind::Method;
        }

        let name_ref = first_child_by_kind(node, "identifier").map(|n| node_text_ref(n, source));
        let own_scope = match name_ref {
            Some(name) => build_scope(scope, ".", name),
            None => scope.to_string(),
        };

        if kinds.contains(&def_kind) {
            if let Some(ident_ref) = name_ref {
                if mode.matches_ident(ident_ref) {
                    let start_row = outer
                        .map(|n| n.start_position().row + 1)
                        .unwrap_or_else(|| node.start_position().row + 1);
                    let start_byte = outer
                        .map(|n| n.start_byte())
                        .unwrap_or_else(|| node.start_byte());
                    let end_byte = match first_child_by_kind(node, "block") {
                        Some(body) => body.start_byte(),
                        None => node.end_byte(),
                    };
                    let raw = flatten_bytes(start_byte, end_byte, source)
                        .unwrap_or_else(|| first_line_of_node(node, source));
                    let signature = clean_signature(&raw);
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

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            collect_definitions(child, source, mode, kinds, results, &own_scope);
        }
        return;
    }

    // TypeAlias annotated assignment: `X: TypeAlias = str | list[str]`
    if kind == "assignment" {
        if let Some(name) = try_extract_typealias_name(node, source) {
            let own_scope = build_scope(scope, ".", &name);
            if kinds.contains(&DefKind::Alias) && mode.matches_ident(&name) {
                let raw = flatten_bytes(node.start_byte(), node.end_byte(), source)
                    .unwrap_or_else(|| first_line_of_node(node, source));
                let signature = clean_signature(&raw);
                let [start, end] = line_range(node.start_position().row + 1, node);
                results.push(DefContent {
                    kind: DefKind::Alias,
                    lines: [start, end],
                    signature,
                    scope: own_scope.clone(),
                });
            }
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                collect_definitions(child, source, mode, kinds, results, &own_scope);
            }
            return;
        }
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_definitions(child, source, mode, kinds, results, scope);
    }
}

/// Clean a Python signature: strip trailing `:`.
///
/// tree-sitter-python's `block` node starts at the first body statement,
/// so `flatten_bytes` includes the `:` delimiter and any comments between
/// `:` and the body. Inline comments are retained as contextual information,
/// consistent with the project convention (see Lua parser).
fn clean_signature(sig: &str) -> String {
    sig.strip_suffix(':').unwrap_or(sig).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::extract_definitions;

    // --- Type alias handler ---

    #[test]
    fn extract_simple_type_alias() {
        let src = "type Point = tuple[float, float]";
        let results = extract_definitions(&PythonParser, "Point", &[DefKind::Alias], src);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].kind, DefKind::Alias);
        assert_eq!(results[0].scope, "Point");
        assert!(results[0].signature.contains("type Point ="));
    }

    #[test]
    fn extract_generic_type_alias() {
        let src = "type Result[T] = T | None";
        let results = extract_definitions(&PythonParser, "Result", &[DefKind::Alias], src);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].scope, "Result");
    }

    #[test]
    fn extract_type_alias_in_class() {
        let src = "class Config:\n    type Value = str | int";
        let results = extract_definitions(&PythonParser, "Value", &[DefKind::Alias], src);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].scope, "Config.Value");
    }

    #[test]
    fn extract_type_alias_in_function() {
        let src = "def factory():\n    type LocalType = int";
        let results = extract_definitions(&PythonParser, "LocalType", &[DefKind::Alias], src);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].scope, "factory.LocalType");
    }

    #[test]
    fn type_alias_not_matched_by_class() {
        let src = "type Point = tuple[float, float]\nclass Point: pass";
        let results = extract_definitions(&PythonParser, "Point", &[DefKind::Alias], src);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].kind, DefKind::Alias);
    }

    #[test]
    fn type_alias_kind_filter() {
        let src = "type Point = tuple[float, float]";
        let results = extract_definitions(&PythonParser, "Point", &[DefKind::Class], src);
        assert!(results.is_empty());
    }

    // --- TypeAlias annotated form ---

    #[test]
    fn extract_typealias_annotated() {
        let src = "HeaderValue: TypeAlias = str | list[str]";
        let results = extract_definitions(&PythonParser, "HeaderValue", &[DefKind::Alias], src);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].kind, DefKind::Alias);
        assert_eq!(results[0].scope, "HeaderValue");
        assert!(results[0].signature.contains("HeaderValue: TypeAlias"));
    }

    #[test]
    fn extract_typealias_annotated_in_class() {
        let src = "class Config:\n    HeaderValue: TypeAlias = str | list[str]";
        let results = extract_definitions(&PythonParser, "HeaderValue", &[DefKind::Alias], src);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].scope, "Config.HeaderValue");
    }

    #[test]
    fn typealias_annotated_kind_filter() {
        let src = "HeaderValue: TypeAlias = str | list[str]";
        let results = extract_definitions(&PythonParser, "HeaderValue", &[DefKind::Class], src);
        assert!(results.is_empty());
    }

    #[test]
    fn plain_annotation_not_detected_as_type() {
        let src = "name: str = \"hello\"";
        let results = extract_definitions(&PythonParser, "name", &[DefKind::Alias], src);
        assert!(results.is_empty());
    }

    #[test]
    fn plain_assignment_not_detected_as_type() {
        let src = "MAX_SIZE = 100";
        let results = extract_definitions(&PythonParser, "MAX_SIZE", &[DefKind::Alias], src);
        assert!(results.is_empty());
    }

    #[test]
    fn type_alias_child_scope_uses_own_scope() {
        // Regression: type_alias_statement should pass &own_scope (not scope)
        // to collect_definitions for child nodes, consistent with function/class.
        // While well-formed Python type aliases don't nest definitions,
        // tree-sitter error recovery may place definition nodes as children.
        // This test verifies scope chain consistency through class -> type_alias.
        let src = "class Container:\n    type Inner = int\n    def method(self): pass";
        // method is a sibling of type_alias_statement in the class body,
        // not a child of type_alias_statement. But if tree-sitter ever
        // nests a definition inside type_alias_statement due to error recovery,
        // it should inherit the type alias scope.
        // Directly: verify method scope is Container.method (correct class scope chain).
        let results = extract_definitions(&PythonParser, "method", &[DefKind::Method], src);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].scope, "Container.method");

        // And verify the type alias itself has correct scope.
        let results = extract_definitions(&PythonParser, "Inner", &[DefKind::Alias], src);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].scope, "Container.Inner");
    }
}
