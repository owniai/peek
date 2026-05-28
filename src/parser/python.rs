use crate::model::{DefContent, DefKind};
use crate::parser::{
    LanguageParser, MatchMode, build_scope, first_child_by_kind, first_line_of_node, flatten_bytes,
    line_range, node_text, node_text_ref,
};
use tree_sitter::{Node, Parser};

pub(crate) const LANGUAGE: &str = "python";
pub(crate) const EXTENSIONS: &[&str] = &["py", "pyw", "pyi", "gyp", "gypi", "wsgi"];
pub(crate) const ALIASES: &[&str] = &["py", "python3"];

pub struct PythonParser;

impl LanguageParser for PythonParser {
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
            DefKind::Constructor,
            DefKind::Destructor,
            DefKind::Getter,
            DefKind::Setter,
            DefKind::Operator,
            DefKind::Subscript,
            DefKind::Class,
            DefKind::Enum,
            DefKind::Protocol,
            DefKind::Alias,
            DefKind::Field,
            DefKind::Var,
            DefKind::Variant,
        ]
    }

    impl_init_parser!(tree_sitter_python::LANGUAGE, "Python");

    impl_extract_with!(collect_definitions, scope: "", in_class: false, in_enum: false);
}

fn kind_for_node(node: Node) -> Option<DefKind> {
    match node.kind() {
        "function_definition" => Some(DefKind::Function),
        "class_definition" => Some(DefKind::Class),
        "type_alias_statement" => Some(DefKind::Alias),
        _ => None,
    }
}

/// Classify a Python class by its base classes.
/// Priority: Enum > Protocol > Class.
fn classify_class_by_base(node: Node, source: &str) -> Option<DefKind> {
    let arg_list = first_child_by_kind(node, "argument_list");
    let arg_list = arg_list?;
    let mut cursor = arg_list.walk();
    for child in arg_list.children(&mut cursor) {
        match child.kind() {
            "identifier" => {
                let name = node_text_ref(child, source);
                if is_enum_base(name) {
                    return Some(DefKind::Enum);
                }
                if name == "Protocol" {
                    return Some(DefKind::Protocol);
                }
            }
            "attribute" => {
                if let Some(attr) = child.child_by_field_name("attribute") {
                    let name = node_text_ref(attr, source);
                    if is_enum_base(name) {
                        return Some(DefKind::Enum);
                    }
                    if name == "Protocol" {
                        return Some(DefKind::Protocol);
                    }
                }
            }
            _ => {}
        }
    }
    None
}

fn is_enum_base(name: &str) -> bool {
    matches!(name, "Enum" | "IntEnum" | "StrEnum" | "Flag" | "IntFlag")
}

/// Classify a Python dunder method by name (only applies inside class body).
fn classify_python_dunder(name: &str) -> Option<DefKind> {
    match name {
        "__init__" | "__new__" => Some(DefKind::Constructor),
        "__del__" => Some(DefKind::Destructor),
        "__getitem__" | "__setitem__" | "__delitem__" | "__missing__" => Some(DefKind::Subscript),
        _ if name.starts_with("__") && name.ends_with("__") && is_operator_dunder(name) => {
            Some(DefKind::Operator)
        }
        _ => None,
    }
}

/// Check if a dunder name corresponds to a Python operator protocol.
fn is_operator_dunder(name: &str) -> bool {
    matches!(
        name,
        // Arithmetic
        "__add__" | "__radd__" | "__iadd__"
        | "__sub__" | "__rsub__" | "__isub__"
        | "__mul__" | "__rmul__" | "__imul__"
        | "__truediv__" | "__rtruediv__" | "__itruediv__"
        | "__floordiv__" | "__rfloordiv__" | "__ifloordiv__"
        | "__mod__" | "__rmod__" | "__imod__"
        | "__pow__" | "__rpow__" | "__ipow__"
        | "__matmul__" | "__rmatmul__" | "__imatmul__"
        | "__divmod__" | "__rdivmod__"
        // Bitwise
        | "__and__" | "__rand__" | "__iand__"
        | "__or__" | "__ror__" | "__ior__"
        | "__xor__" | "__rxor__" | "__ixor__"
        | "__lshift__" | "__rlshift__" | "__ilshift__"
        | "__rshift__" | "__rrshift__" | "__irshift__"
        // Unary
        | "__neg__" | "__pos__" | "__abs__" | "__invert__"
        // Comparison
        | "__eq__" | "__ne__" | "__lt__" | "__le__" | "__gt__" | "__ge__"
        | "__cmp__"
        // Conversion
        | "__hash__" | "__bool__" | "__len__" | "__contains__"
        | "__int__" | "__float__" | "__complex__" | "__round__"
        | "__trunc__" | "__floor__" | "__ceil__" | "__index__"
        | "__bytes__" | "__str__" | "__repr__" | "__format__"
        // Iterator
        | "__iter__" | "__next__" | "__reversed__"
        | "__length_hint__"
        // Callable / context manager
        | "__call__" | "__enter__" | "__exit__"
        | "__await__" | "__aenter__" | "__aexit__"
        // Async iterator
        | "__aiter__" | "__anext__"
    )
}

/// Check if a decorated_definition node wraps a property decorator.
/// Returns Some(Getter) for @property / @name.getter, Some(Setter) for @name.setter.
fn check_property_decorator(outer: Option<Node>, source: &str) -> Option<DefKind> {
    let decorated = outer?;
    let mut cursor = decorated.walk();
    for child in decorated.children(&mut cursor) {
        if child.kind() != "decorator" {
            continue;
        }
        // decorator content: check for @property or @name.setter/@name.getter
        let text = node_text(child, source);
        if text == "@property" {
            return Some(DefKind::Getter);
        }
        if let Some(rest) = text.strip_prefix('@') {
            // Check for name.setter or name.getter or name.deleter
            if let Some(dot_pos) = rest.rfind('.') {
                let suffix = &rest[dot_pos + 1..];
                match suffix {
                    "setter" | "deleter" => return Some(DefKind::Setter),
                    "getter" => return Some(DefKind::Getter),
                    _ => {}
                }
            }
        }
    }
    None
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

#[allow(clippy::too_many_arguments)]
fn collect_definitions<'a>(
    node: Node<'a>,
    source: &str,
    mode: &MatchMode,
    kinds: &[DefKind],
    results: &mut Vec<DefContent>,
    scope: &str,
    in_class: bool,
    in_enum: bool,
) {
    let kind = node.kind();

    if kind == "decorated_definition" {
        if let Some(inner) = first_child_by_kind(node, "function_definition") {
            try_add_definition(
                inner,
                source,
                mode,
                kinds,
                results,
                scope,
                Some(node),
                in_class,
                in_enum,
            );
        }
        if let Some(inner) = first_child_by_kind(node, "class_definition") {
            try_add_definition(
                inner,
                source,
                mode,
                kinds,
                results,
                scope,
                Some(node),
                in_class,
                in_enum,
            );
        }
        return;
    }

    try_add_definition(
        node, source, mode, kinds, results, scope, None, in_class, in_enum,
    );
}

#[allow(clippy::too_many_arguments)]
fn try_add_definition<'a>(
    node: Node<'a>,
    source: &str,
    mode: &MatchMode,
    kinds: &[DefKind],
    results: &mut Vec<DefContent>,
    scope: &str,
    outer: Option<Node<'a>>,
    in_class: bool,
    in_enum: bool,
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
                collect_definitions(
                    child, source, mode, kinds, results, &own_scope, in_class, in_enum,
                );
            }
            return;
        }

        // function_definition / class_definition
        let mut def_kind = kind_for_node(node).unwrap();

        // Classify class by base classes: Enum/Protocol override Class
        if kind == "class_definition" {
            if let Some(base_kind) = classify_class_by_base(node, source) {
                def_kind = base_kind;
            }
        }

        // In Python, scope is only set by class_definition, so !scope.is_empty()
        // means this function_definition is inside a class body → Method
        if kind == "function_definition" && !scope.is_empty() {
            def_kind = DefKind::Method;

            // Check @property decorator first (highest priority)
            if let Some(decorator_kind) = check_property_decorator(outer, source) {
                def_kind = decorator_kind;
            } else if let Some(name_text) =
                first_child_by_kind(node, "identifier").map(|n| node_text_ref(n, source))
            {
                // Then check dunder method names
                if let Some(dunder_kind) = classify_python_dunder(name_text) {
                    def_kind = dunder_kind;
                }
            }
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

        let is_class = kind == "class_definition";
        let is_enum_class = def_kind == DefKind::Enum;

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            collect_definitions(
                child,
                source,
                mode,
                kinds,
                results,
                &own_scope,
                in_class || is_class,
                is_enum_class,
            );
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
                collect_definitions(
                    child, source, mode, kinds, results, &own_scope, in_class, in_enum,
                );
            }
            return;
        }

        // Class/Enum body assignment: `name = 'default'` → Field or Variant
        // Only when inside a class body (not function body)
        // Enum members → Variant, regular class members → Field
        if in_class {
            let member_kind = if in_enum {
                DefKind::Variant
            } else {
                DefKind::Field
            };
            if kinds.contains(&member_kind) {
                if let Some(left) = node.child_by_field_name("left") {
                    if left.kind() == "identifier" {
                        let name_ref = node_text_ref(left, source);
                        if mode.matches_ident(name_ref) {
                            let name = name_ref.to_string();
                            let own_scope = build_scope(scope, ".", &name);
                            let mut cursor = node.walk();
                            let eq_byte = node
                                .children(&mut cursor)
                                .find(|c| c.kind() == "=")
                                .map(|c| c.start_byte())
                                .unwrap_or_else(|| node.end_byte());
                            let raw = flatten_bytes(node.start_byte(), eq_byte, source)
                                .unwrap_or_else(|| first_line_of_node(node, source));
                            let signature = clean_signature(&raw);
                            let [start, end] = line_range(node.start_position().row + 1, node);
                            results.push(DefContent {
                                kind: member_kind,
                                lines: [start, end],
                                signature,
                                scope: own_scope,
                            });
                        }
                    }
                }
            }
        }

        // Module-level simple assignment: `x = 1` (only at top level, scope is empty)
        if scope.is_empty() && kinds.contains(&DefKind::Var) {
            if let Some(left) = node.child_by_field_name("left") {
                if left.kind() == "identifier" {
                    let name_ref = node_text_ref(left, source);
                    if mode.matches_ident(name_ref) {
                        let name = name_ref.to_string();
                        let own_scope = name.clone();
                        // Truncate signature to the = sign
                        let mut cursor = node.walk();
                        let eq_byte = node
                            .children(&mut cursor)
                            .find(|c| c.kind() == "=")
                            .map(|c| c.start_byte())
                            .unwrap_or_else(|| node.end_byte());
                        let raw = flatten_bytes(node.start_byte(), eq_byte, source)
                            .unwrap_or_else(|| first_line_of_node(node, source));
                        let signature = clean_signature(&raw);
                        let [start, end] = line_range(node.start_position().row + 1, node);
                        results.push(DefContent {
                            kind: DefKind::Var,
                            lines: [start, end],
                            signature,
                            scope: own_scope,
                        });
                    }
                }
            }
        }
        // Do not recurse into assignment body (prevents function-body vars)
        return;
    }

    // Annotated assignment: `x: int = 1` or `x: str`
    if kind == "annotated_assignment" {
        // Check if this is a TypeAlias annotation (already handled above for assignment)
        // annotated_assignment uses `type` field for the annotation, not `type` child
        let type_node = node.child_by_field_name("type");
        let is_typealias = type_node
            .as_ref()
            .map(|t| is_typealias_annotation(t, source))
            .unwrap_or(false);

        // Class/Enum body annotated assignment: `timeout: int = 30` → Field or Variant
        // Enum members → Variant, regular class members → Field
        if in_class && !is_typealias {
            let member_kind = if in_enum {
                DefKind::Variant
            } else {
                DefKind::Field
            };
            if kinds.contains(&member_kind) {
                if let Some(left) = node.child_by_field_name("left") {
                    if left.kind() == "identifier" {
                        let name_ref = node_text_ref(left, source);
                        if mode.matches_ident(name_ref) {
                            let name = name_ref.to_string();
                            let own_scope = build_scope(scope, ".", &name);
                            let raw = flatten_bytes(node.start_byte(), node.end_byte(), source)
                                .unwrap_or_else(|| first_line_of_node(node, source));
                            let signature = clean_signature(&raw);
                            let [start, end] = line_range(node.start_position().row + 1, node);
                            results.push(DefContent {
                                kind: member_kind,
                                lines: [start, end],
                                signature,
                                scope: own_scope,
                            });
                        }
                    }
                }
            }
        }

        // Module-level annotated assignment: `x: int = 1` → Var
        if scope.is_empty() && kinds.contains(&DefKind::Var) && !is_typealias {
            if let Some(left) = node.child_by_field_name("left") {
                if left.kind() == "identifier" {
                    let name_ref = node_text_ref(left, source);
                    if mode.matches_ident(name_ref) {
                        let name = name_ref.to_string();
                        let own_scope = name.clone();
                        let raw = flatten_bytes(node.start_byte(), node.end_byte(), source)
                            .unwrap_or_else(|| first_line_of_node(node, source));
                        let signature = clean_signature(&raw);
                        let [start, end] = line_range(node.start_position().row + 1, node);
                        results.push(DefContent {
                            kind: DefKind::Var,
                            lines: [start, end],
                            signature,
                            scope: own_scope,
                        });
                    }
                }
            }
        }
        return;
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_definitions(
            child, source, mode, kinds, results, scope, in_class, in_enum,
        );
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
    use crate::model::DefKind;

    fn extract(source: &str) -> Vec<DefContent> {
        let parser = PythonParser;
        let mut ts_parser = parser.init_parser();
        let all_kinds: Vec<DefKind> = parser.supported_kinds().to_vec();
        parser
            .extract_with(&MatchMode::All, &all_kinds, source, &mut ts_parser)
            .unwrap()
    }

    #[test]
    fn decorator_on_function_sets_start_row() {
        let source = "\n@cache\ndef foo():\n    pass\n";
        let defs = extract(source);
        let foo = defs.iter().find(|d| d.scope == "foo").unwrap();
        assert_eq!(
            foo.lines[0], 2,
            "start line should be the @cache decorator line"
        );
    }

    #[test]
    fn decorator_on_class_sets_start_row() {
        let source = "\n@dataclass\nclass Foo:\n    x: int\n";
        let defs = extract(source);
        let foo = defs.iter().find(|d| d.scope == "Foo").unwrap();
        assert_eq!(
            foo.lines[0], 2,
            "start line should be the @dataclass decorator line"
        );
    }

    #[test]
    fn no_decorator_uses_node_start_row() {
        let source = "\ndef bar():\n    pass\n";
        let defs = extract(source);
        let bar = defs.iter().find(|d| d.scope == "bar").unwrap();
        assert_eq!(bar.lines[0], 2, "start line should be the def line");
    }
}
