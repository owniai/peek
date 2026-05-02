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
        "operator_signature" => {
            handle_operator_sig(node, source, mode, kinds, results, scope);
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

    let signature = extract_signature_to_body(node, source);
    let start_row = (node.start_position().row + 1) as u32;
    let end_row = dart_end_row(node);

    let own_scope = build_scope_from_node(node, source, scope, ".");
    results.push(DefContent {
        kind: DefKind::Function,
        lines: [start_row, end_row],
        signature,
        scope: own_scope,
    });
}

/// Handle operator_signature (Dart operator overloading).
/// Node structure: operator_signature → type_identifier, operator, *_operator, formal_parameter_list
fn handle_operator_sig(
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

    let operator_name = extract_operator_name(node, source);
    if !mode.matches_ident(&operator_name) {
        return;
    }

    let own_scope = build_scope(scope, ".", &operator_name);
    let signature = first_line_of_node(node, source);
    let signature = normalize_signature(&signature);
    let start_row = (node.start_position().row + 1) as u32;
    let end_row = dart_end_row(node);

    results.push(DefContent {
        kind: DefKind::Function,
        lines: [start_row, end_row],
        signature,
        scope: own_scope,
    });
}

/// Extract operator name from operator_signature node.
/// Finds the *_operator child (e.g., binary_operator "+") and returns "operator+".
fn extract_operator_name(node: Node, source: &str) -> String {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.is_named() && child.kind().ends_with("_operator") {
            let symbol = node_text_ref(child, source);
            return format!("operator{}", symbol);
        }
    }
    "operator".to_string()
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
    let start_row = (node.start_position().row + 1) as u32;
    let end_row = dart_end_row(node);

    results.push(DefContent {
        kind: DefKind::Function,
        lines: [start_row, end_row],
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

                    if mode.matches_ident(match_name) {
                        let signature = extract_signature_to_body(node, source);
                        let start_row = (node.start_position().row + 1) as u32;
                        let end_row = dart_end_row(node);

                        results.push(DefContent {
                            kind: DefKind::Function,
                            lines: [start_row, end_row],
                            signature,
                            scope: ctor_scope,
                        });
                    }
                }
                "function_signature" => {
                    // Abstract method inside class (declaration > function_signature)
                    if let Some(name_node) = child.child_by_field_name("name") {
                        let name_ref = node_text_ref(name_node, source);
                        if mode.matches_ident(name_ref) {
                            let own_scope = build_scope(scope, ".", name_ref);
                            let signature = extract_signature_to_body(node, source);
                            let start_row = (node.start_position().row + 1) as u32;
                            let end_row = dart_end_row(node);

                            results.push(DefContent {
                                kind: DefKind::Function,
                                lines: [start_row, end_row],
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
    fn test_mixin_class_found_via_both_class_and_mixin_kind() {
        // Dart 3 "mixin class" is both a class and a mixin semantically.
        // tree-sitter-dart parses it as class_declaration with a "mixin" named child.
        // Both -k class and -k mixin should find it.
        let source = r#"
mixin class Draggable {
  void drag() {}
}
"#;
        // Searching as Class kind — should find it
        let class_defs = extract_definitions(&DartParser, "Draggable", &[DefKind::Class], source);
        assert_eq!(
            class_defs.len(),
            1,
            "mixin class should be found via Class kind"
        );
        assert_eq!(class_defs[0].kind, DefKind::Class);

        // Searching as Mixin kind — should also find it
        let mixin_defs = extract_definitions(&DartParser, "Draggable", &[DefKind::Mixin], source);
        assert_eq!(
            mixin_defs.len(),
            1,
            "mixin class should be found via Mixin kind"
        );
        assert_eq!(mixin_defs[0].kind, DefKind::Mixin);

        // Both results share the same location
        assert_eq!(class_defs[0].lines, mixin_defs[0].lines);
    }

    #[test]
    fn test_plain_class_not_found_via_mixin_kind() {
        // A plain class (without mixin modifier) should NOT be found via Mixin kind
        let source = r#"
class PlainClass {
  void method() {}
}
"#;
        let defs = extract_definitions(&DartParser, "PlainClass", &[DefKind::Mixin], source);
        assert!(
            defs.is_empty(),
            "plain class should not be found via Mixin kind"
        );
    }

    #[test]
    fn test_binary_operator_found() {
        let source = r#"
class Vector {
  final int x, y;
  Vector(this.x, this.y);
  Vector operator +(Vector other) => Vector(x + other.x, y + other.y);
}
"#;
        let defs = extract_definitions(&DartParser, "operator", &[DefKind::Function], source);
        assert_eq!(defs.len(), 1, "operator+ should be extracted");
        assert_eq!(defs[0].kind, DefKind::Function);
        assert_eq!(defs[0].scope, "Vector.operator+");
        assert!(defs[0].signature.contains("operator"));
    }

    #[test]
    fn test_comparison_operator_found() {
        let source = r#"
class Money {
  final int amount;
  Money(this.amount);
  bool operator ==(Object other) => other is Money && amount == other.amount;
}
"#;
        let defs = extract_definitions(&DartParser, "operator", &[DefKind::Function], source);
        assert_eq!(defs.len(), 1, "operator== should be extracted");
        assert_eq!(defs[0].scope, "Money.operator==");
    }

    #[test]
    fn test_operator_kind_filtered_out() {
        let source = r#"
class Vector {
  Vector operator +(Vector other) => Vector(0, 0);
}
"#;
        let defs = extract_definitions(&DartParser, "operator", &[DefKind::Class], source);
        assert!(
            defs.is_empty(),
            "operator should not be found when kind is Class"
        );
    }

    #[test]
    fn test_named_constructor_found_by_name() {
        // In Dart, MyClass.named(int x) is a named constructor.
        // tree-sitter-dart represents it as constructor_signature with TWO identifier children:
        //   identifier "MyClass" (class name) and identifier "named" (constructor name).
        let source = r#"
class Point {
  final int x, y;
  Point(this.x, this.y);
  Point.origin() : x = 0, y = 0;
}
"#;
        // Search for named constructor "origin" — should find it
        let defs = extract_definitions(&DartParser, "origin", &[DefKind::Function], source);
        assert_eq!(defs.len(), 1, "named constructor 'origin' should be found");
        assert_eq!(defs[0].scope, "Point.origin");
        assert!(defs[0].signature.contains("origin"));

        // Default constructor should still work — search by class name "Point"
        let default_defs = extract_definitions(&DartParser, "Point", &[DefKind::Function], source);
        // Both constructors match: default (name=Point) and named (class=Point for scope)
        // The default constructor matches on "Point" directly
        assert!(
            !default_defs.is_empty(),
            "default constructor should be found via class name"
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
}
