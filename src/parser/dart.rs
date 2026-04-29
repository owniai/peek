use crate::model::{DefContent, DefKind};
use crate::parser::{
    LanguageParser, MatchMode, build_scope, build_scope_from_node, extract_signature_to_body,
    first_child_by_kind, first_line_of_node, line_range, node_text_ref, normalize_signature,
};
use tree_sitter::{Node, Parser};

pub struct DartParser;

impl LanguageParser for DartParser {
    fn language(&self) -> &'static str {
        "dart"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &[".dart"]
    }

    fn supported_kinds(&self) -> &'static [DefKind] {
        &[
            DefKind::Function,
            DefKind::Class,
            DefKind::Enum,
            DefKind::Const,
            DefKind::Type,
            DefKind::Mixin,
            DefKind::Extension,
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
        "class_declaration" => {
            handle_class(node, source, mode, kinds, results, scope);
        }
        "enum_declaration" => {
            handle_enum(node, source, mode, kinds, results, scope);
        }
        "mixin_declaration" => {
            handle_mixin(node, source, mode, kinds, results, scope);
        }
        "extension_declaration" | "extension_type_declaration" => {
            handle_extension(node, source, mode, kinds, results, scope);
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
        "getter_signature" | "setter_signature" => {
            handle_accessor(node, source, mode, kinds, results, scope);
        }
        "static_final_declaration_list" => {
            handle_const_list(node, source, mode, kinds, results, scope);
        }
        "declaration" => {
            handle_declaration(node, source, mode, kinds, results, scope);
        }
        "class_member" | "method_signature" => {
            recurse_children(node, source, mode, kinds, results, scope);
        }
        _ => {
            recurse_children(node, source, mode, kinds, results, scope);
        }
    }
}

/// Handle class_declaration: extract class definition and recurse into class_body.
fn handle_class(
    node: Node,
    source: &str,
    mode: &MatchMode,
    kinds: &[DefKind],
    results: &mut Vec<DefContent>,
    scope: &str,
) {
    let own_scope = build_scope_from_node(node, source, scope, ".");
    if kinds.contains(&DefKind::Class) {
        if let Some(name_node) = node.child_by_field_name("name") {
            let name_ref = node_text_ref(name_node, source);
            if mode.matches_ident(name_ref) {
                let signature = extract_signature_to_body(node, source);
                let start_row = node.start_position().row + 1;
                let [start, end] = line_range(start_row, node);

                results.push(DefContent {
                    kind: DefKind::Class,
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

/// Handle type_alias: extract typedef definition.
fn handle_type_alias(
    node: Node,
    source: &str,
    mode: &MatchMode,
    kinds: &[DefKind],
    results: &mut Vec<DefContent>,
    scope: &str,
) {
    if !kinds.contains(&DefKind::Type) {
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
                kind: DefKind::Type,
                lines: [start, end],
                signature,
                scope: own_scope,
            });
        }
    }
}

/// Handle top-level function_signature (paired with function_body as sibling).
fn handle_function_sig(
    node: Node,
    source: &str,
    mode: &MatchMode,
    kinds: &[DefKind],
    results: &mut Vec<DefContent>,
    scope: &str,
) {
    if !kinds.contains(&DefKind::Function) {
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

    // For top-level function_signature, the signature spans from function_signature
    // to the start of the sibling function_body (or end of node if no body sibling)
    let signature = extract_signature_to_body(node, source);
    let start_row = node.start_position().row + 1;
    // For top-level functions, end line includes the function_body sibling
    // We use function_signature node for line_range; the sibling function_body extends coverage
    let [start, end] = line_range(start_row, node);

    let own_scope = build_scope_from_node(node, source, scope, ".");
    results.push(DefContent {
        kind: DefKind::Function,
        lines: [start, end],
        signature,
        scope: own_scope,
    });
}

/// Handle getter_signature and setter_signature at top level.
fn handle_accessor(
    node: Node,
    source: &str,
    mode: &MatchMode,
    kinds: &[DefKind],
    results: &mut Vec<DefContent>,
    scope: &str,
) {
    if !kinds.contains(&DefKind::Function) {
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
    let signature = normalize_signature(&signature);
    let start_row = node.start_position().row + 1;
    let [start, end] = line_range(start_row, node);

    results.push(DefContent {
        kind: DefKind::Function,
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

    // Check for method signatures inside declaration (e.g., constructor)
    // For constructor_signature, we treat it as a Function
    if kinds.contains(&DefKind::Function) {
        for child in children.iter().copied() {
            match child.kind() {
                "constructor_signature"
                | "constant_constructor_signature"
                | "factory_constructor_signature" => {
                    // Extract constructor name: these nodes have no "name" field,
                    // first identifier child is the class name
                    if let Some(name_node) = first_child_by_kind(child, "identifier") {
                        let name_ref = node_text_ref(name_node, source);
                        if mode.matches_ident(name_ref) {
                            let own_scope = build_scope(scope, ".", name_ref);
                            let signature = extract_signature_to_body(node, source);
                            let start_row = node.start_position().row + 1;
                            let [start, end] = line_range(start_row, node);

                            results.push(DefContent {
                                kind: DefKind::Function,
                                lines: [start, end],
                                signature,
                                scope: own_scope,
                            });
                        }
                    }
                }
                "function_signature" => {
                    // Abstract method inside class (declaration > function_signature)
                    if let Some(name_node) = child.child_by_field_name("name") {
                        let name_ref = node_text_ref(name_node, source);
                        if mode.matches_ident(name_ref) {
                            let own_scope = build_scope(scope, ".", name_ref);
                            let signature = extract_signature_to_body(node, source);
                            let start_row = node.start_position().row + 1;
                            let [start, end] = line_range(start_row, node);

                            results.push(DefContent {
                                kind: DefKind::Function,
                                lines: [start, end],
                                signature,
                                scope: own_scope,
                            });
                        }
                    }
                }
                _ => {}
            }
        }
    }
}

// === Helper functions ===

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::extract_definitions;

    /// Extract all definitions using all supported kinds with a wildcard match.
    fn parse_all(source: &str) -> Vec<DefContent> {
        let parser = DartParser;
        let mut ts_parser = parser.init_parser();
        let mode = MatchMode::Exact {
            name: "*".to_string(),
            case_insensitive: false,
        };
        let all_kinds = parser.supported_kinds().to_vec();
        parser
            .extract_with(&mode, &all_kinds, source, &mut ts_parser)
            .unwrap()
    }

    // --- Meta tests ---

    #[test]
    fn test_language() {
        let parser = DartParser;
        assert_eq!(parser.language(), "dart");
    }

    #[test]
    fn test_extensions() {
        let parser = DartParser;
        assert_eq!(parser.extensions(), &[".dart"]);
    }

    #[test]
    fn test_supported_kinds() {
        let parser = DartParser;
        let kinds = parser.supported_kinds();
        assert!(kinds.contains(&DefKind::Function));
        assert!(kinds.contains(&DefKind::Class));
        assert!(kinds.contains(&DefKind::Enum));
        assert!(kinds.contains(&DefKind::Const));
        assert!(kinds.contains(&DefKind::Type));
        assert!(kinds.contains(&DefKind::Mixin));
        assert!(kinds.contains(&DefKind::Extension));
    }

    // --- Edge case / handler tests ---

    #[test]
    fn test_unnamed_extension_not_extracted() {
        let source = r#"
extension on String {
  String trimmed() => trim();
}
"#;
        // Unnamed extension has no "name" field, should not be extracted
        let defs = extract_definitions(&DartParser, "String", &[DefKind::Extension], source);
        // "String" is the `on` type, not the extension name
        assert!(defs.is_empty());
    }

    #[test]
    fn test_kind_filter_excludes_unrelated() {
        let source = r#"
class MyClass {
  void myMethod() {}
}
"#;
        let defs = extract_definitions(&DartParser, "MyClass", &[DefKind::Function], source);
        assert!(defs.is_empty());
    }

    #[test]
    fn test_empty_source() {
        let source = "";
        let defs = extract_definitions(&DartParser, "anything", &[DefKind::Function], source);
        assert!(defs.is_empty());
    }

    // --- Bug verification tests ---

    #[test]
    fn test_mixin_class_not_found_via_mixin_kind() {
        // Dart 3 "mixin class" is a class that can also be used as a mixin.
        // tree-sitter-dart parses it as class_declaration with a "mixin" anonymous child.
        // The regex for DefKind::Mixin is "(?:base\s+)?mixin\s+" which does NOT match "mixin class".
        // So "mixin class Draggable" cannot be found when searching for Mixin kind.
        // It CAN be found as Class kind (because the class regex's * allows zero matches,
        // matching "class Draggable" inside "mixin class Draggable").
        let source = r#"
mixin class Draggable {
  void drag() {}
}
"#;
        // Searching as Mixin kind — fails because regex is "(?:base\s+)?mixin\s+" which doesn't match "mixin class"
        let defs = extract_definitions(&DartParser, "Draggable", &[DefKind::Mixin], source);
        assert_eq!(
            defs.len(),
            0,
            "mixin class should not be found via Mixin regex since 'mixin class' != 'mixin '"
        );
    }

    #[test]
    fn test_operator_method_not_found() {
        // Dart operator overloading uses `operatorSignature` in tree-sitter-dart.
        // In the AST: method_signature -> operator_signature (not function_signature).
        // The parser's handle_function_sig only matches "function_signature",
        // so operator methods are silently skipped.
        let source = r#"
class Vector {
  final int x, y;
  Vector(this.x, this.y);
  Vector operator +(Vector other) => Vector(x + other.x, y + other.y);
}
"#;
        // operator+ is a method of Vector, should be found as Function
        let defs = parse_all(source);
        let operator_defs: Vec<&DefContent> = defs
            .iter()
            .filter(|d| d.signature.contains("operator"))
            .collect();
        // BUG: operator methods are NOT extracted because the parser doesn't handle operator_signature
        assert!(
            operator_defs.is_empty(),
            "operator methods should NOT be extracted (bug confirmed)"
        );
    }

    #[test]
    fn test_named_constructor_cannot_be_found_by_name() {
        // In Dart, MyClass.named(int x) is a named constructor.
        // tree-sitter-dart represents it as constructor_signature with TWO identifier children:
        //   identifier "MyClass" (class name) and identifier "named" (constructor name).
        // The parser's handle_declaration uses first_child_by_kind(child, "identifier")
        // which gets the FIRST identifier (class name "MyClass"), ignoring the constructor name.
        // So searching for the named constructor "named" will NOT find it.
        let source = r#"
class Point {
  final int x, y;
  Point(this.x, this.y);
  Point.origin() : x = 0, y = 0;
}
"#;
        // Search for named constructor "origin" — should find it but doesn't
        let defs = extract_definitions(&DartParser, "origin", &[DefKind::Function], source);
        assert_eq!(
            defs.len(),
            0,
            "named constructor 'origin' cannot be found (bug confirmed)"
        );
    }

    #[test]
    fn test_abstract_method_not_found_in_class() {
        let source = r#"
abstract class Shape {
  double area();
  double perimeter();
  String describe() => "Shape";
}
"#;
        // "describe" is a concrete method (has body) - should be found
        let concrete_defs =
            extract_definitions(&DartParser, "describe", &[DefKind::Function], source);
        assert_eq!(concrete_defs.len(), 1, "concrete method should be found");
        assert_eq!(concrete_defs[0].scope, "Shape.describe");

        // "area" is an abstract method (no body) - should be found via function_signature
        let abstract_defs = extract_definitions(&DartParser, "area", &[DefKind::Function], source);
        assert_eq!(
            abstract_defs.len(),
            1,
            "abstract method 'area' should be extracted"
        );
        assert_eq!(abstract_defs[0].scope, "Shape.area");

        // "perimeter" is also abstract - should be found
        let perimeter_defs =
            extract_definitions(&DartParser, "perimeter", &[DefKind::Function], source);
        assert_eq!(
            perimeter_defs.len(),
            1,
            "abstract method 'perimeter' should be extracted"
        );
        assert_eq!(perimeter_defs[0].scope, "Shape.perimeter");
    }

    #[test]
    fn test_abstract_vs_concrete_method_extraction() {
        let source = r#"
abstract class Repository {
  Future<Data> findById(int id);
  void delete(int id);
  void log(String msg) {}
}
"#;
        // Concrete method "log" (has function body) - should be found
        let log_defs = extract_definitions(&DartParser, "log", &[DefKind::Function], source);
        assert_eq!(log_defs.len(), 1, "concrete method 'log' should be found");
        assert_eq!(log_defs[0].scope, "Repository.log");

        // Abstract method "findById" (no body, semicolon only) - should be found
        let find_defs = extract_definitions(&DartParser, "findById", &[DefKind::Function], source);
        assert_eq!(
            find_defs.len(),
            1,
            "abstract method 'findById' should be extracted"
        );
        assert_eq!(find_defs[0].scope, "Repository.findById");

        // Abstract method "delete" (no body) - should be found
        let delete_defs = extract_definitions(&DartParser, "delete", &[DefKind::Function], source);
        assert_eq!(
            delete_defs.len(),
            1,
            "abstract method 'delete' should be extracted"
        );
        assert_eq!(delete_defs[0].scope, "Repository.delete");
    }

    // === DIAGNOSTIC: dump AST node types for key Dart constructs ===

    fn dump_tree(node: tree_sitter::Node, source: &str, depth: usize) {
        let indent = "  ".repeat(depth);
        let text = node.utf8_text(source.as_bytes()).unwrap_or("?");
        let short_text = if text.len() > 60 { &text[..60] } else { text };
        let field_name = node
            .field_name_for_child(0)
            .map(|f| format!("[{}] ", f))
            .unwrap_or_default();
        eprintln!(
            "{}{}{} ({}) field_name_for_child_0={:?}",
            indent,
            field_name,
            node.kind(),
            short_text.replace('\n', "\\n"),
            node.field_name_for_child(0)
        );
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            dump_tree(child, source, depth + 1);
        }
    }

    #[test]
    fn diag_dump_mixin_class_ast() {
        let source = r#"
mixin class Draggable {
  void drag() {}
}
"#;
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_dart::LANGUAGE.into())
            .unwrap();
        let tree = parser.parse(source, None).unwrap();
        eprintln!("=== mixin class AST ===");
        dump_tree(tree.root_node(), source, 0);
    }

    #[test]
    fn diag_dump_named_constructor_ast() {
        let source = r#"
class MyClass {
  MyClass.named(int x);
}
"#;
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_dart::LANGUAGE.into())
            .unwrap();
        let tree = parser.parse(source, None).unwrap();
        eprintln!("=== named constructor AST ===");
        dump_tree(tree.root_node(), source, 0);
    }

    #[test]
    fn diag_dump_operator_ast() {
        let source = r#"
class Vector {
  final int x, y;
  Vector(this.x, this.y);
  Vector operator +(Vector other) => Vector(x + other.x, y + other.y);
}
"#;
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_dart::LANGUAGE.into())
            .unwrap();
        let tree = parser.parse(source, None).unwrap();
        eprintln!("=== operator overloading AST ===");
        dump_tree(tree.root_node(), source, 0);
    }

    #[test]
    fn diag_dump_getter_setter_top_level_ast() {
        let source = r#"
int get topLevelGetter => 42;
set topLevelSetter(int value) {}
"#;
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_dart::LANGUAGE.into())
            .unwrap();
        let tree = parser.parse(source, None).unwrap();
        eprintln!("=== top-level getter/setter AST ===");
        dump_tree(tree.root_node(), source, 0);
    }

    #[test]
    fn diag_dump_enum_with_method_ast() {
        let source = r#"
enum Status {
  active, inactive, pending;

  String label() => name;
}
"#;
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_dart::LANGUAGE.into())
            .unwrap();
        let tree = parser.parse(source, None).unwrap();
        eprintln!("=== enum with method AST ===");
        dump_tree(tree.root_node(), source, 0);
    }
}
