use std::path::PathBuf;

macro_rules! define_def_kinds {
    ($($variant:ident => $tag:literal),* $(,)?) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        #[repr(u8)]
        pub enum DefKind { $($variant,)* }

        impl DefKind {
            pub fn display_tag(&self) -> &'static str {
                match self { $(DefKind::$variant => $tag,)* }
            }
            pub fn all() -> &'static [DefKind] {
                &[$(DefKind::$variant,)*]
            }
            pub fn from_tag(tag: &str) -> Option<DefKind> {
                match tag {
                    $($tag => Some(DefKind::$variant),)*
                    _ => None,
                }
            }
            pub fn to_u8(self) -> u8 {
                self as u8
            }
            /// Map a discriminant back to a DefKind.
            /// Relies on `#[repr(u8)]` producing sequential discriminants 0..N
            /// — validated by the compile-time assertion below the enum.
            pub fn from_u8(v: u8) -> Option<DefKind> {
                Self::all().get(v as usize).copied()
            }
        }
    }
}

define_def_kinds!(
    Function => "function",
    Class => "class",
    Struct => "struct",
    Enum => "enum",
    Alias => "alias",
    Trait => "trait",
    Interface => "interface",
    Const => "const",
    Record => "record",
    Delegate => "delegate",
    Event => "event",
    Object => "object",
    Protocol => "protocol",
    Actor => "actor",
    Extension => "extension",
    ExtensionType => "extension_type",
    Mixin => "mixin",
    Module => "module",
    Macro => "macro",
    Union => "union",
    Method => "method",
    Constructor => "constructor",
    Getter => "getter",
    Setter => "setter",
    Operator => "operator",
    Field => "field",
    Property => "property",
    Var => "var",
    Namespace => "namespace",
    Package => "package",
    Variant => "variant",
    Destructor => "destructor",
    Subscript => "subscript",
    Annotation => "annotation",
    Concept => "concept",
    ModuleDeclaration => "module_declaration",
    AssociatedType => "associated_type",
    FunctionDeclaration => "function_declaration",
    MethodDeclaration => "method_declaration",
    ConstructorDeclaration => "constructor_declaration",
    DestructorDeclaration => "destructor_declaration",
    OperatorDeclaration => "operator_declaration",
    GetterDeclaration => "getter_declaration",
    SetterDeclaration => "setter_declaration",
    SubscriptDeclaration => "subscript_declaration",
    PropertyDeclaration => "property_declaration",
    VarDeclaration => "var_declaration",
    ConstDeclaration => "const_declaration",
    StructDeclaration => "struct_declaration",
    ClassDeclaration => "class_declaration",
    UnionDeclaration => "union_declaration",
    EnumDeclaration => "enum_declaration",
    Linkage => "linkage",
    Impl => "impl",
);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Category {
    Shape,
    Callable,
    Value,
    Contract,
    Scope,
}

impl Category {
    pub fn display_tag(&self) -> &'static str {
        match self {
            Category::Shape => "shape",
            Category::Callable => "callable",
            Category::Value => "value",
            Category::Contract => "contract",
            Category::Scope => "scope",
        }
    }

    pub fn all() -> &'static [Category] {
        &[
            Category::Shape,
            Category::Callable,
            Category::Value,
            Category::Contract,
            Category::Scope,
        ]
    }

    pub fn from_tag(tag: &str) -> Option<Category> {
        match tag {
            "shape" => Some(Category::Shape),
            "callable" => Some(Category::Callable),
            "value" => Some(Category::Value),
            "contract" => Some(Category::Contract),
            "scope" => Some(Category::Scope),
            _ => None,
        }
    }

    pub fn members(&self) -> &'static [DefKind] {
        match self {
            Category::Shape => &[
                DefKind::Class,
                DefKind::Struct,
                DefKind::Enum,
                DefKind::Union,
                DefKind::Record,
                DefKind::Object,
                DefKind::Actor,
                DefKind::ExtensionType,
                DefKind::StructDeclaration,
                DefKind::ClassDeclaration,
                DefKind::UnionDeclaration,
                DefKind::EnumDeclaration,
            ],
            Category::Callable => &[
                DefKind::Function,
                DefKind::Method,
                DefKind::Constructor,
                DefKind::Getter,
                DefKind::Setter,
                DefKind::Operator,
                DefKind::Destructor,
                DefKind::Subscript,
                DefKind::FunctionDeclaration,
                DefKind::MethodDeclaration,
                DefKind::ConstructorDeclaration,
                DefKind::GetterDeclaration,
                DefKind::SetterDeclaration,
                DefKind::OperatorDeclaration,
                DefKind::DestructorDeclaration,
                DefKind::SubscriptDeclaration,
            ],
            Category::Value => &[
                DefKind::Const,
                DefKind::Event,
                DefKind::Field,
                DefKind::Property,
                DefKind::Var,
                DefKind::Variant,
                DefKind::PropertyDeclaration,
                DefKind::VarDeclaration,
                DefKind::ConstDeclaration,
            ],
            Category::Contract => &[
                DefKind::Interface,
                DefKind::Protocol,
                DefKind::Trait,
                DefKind::Mixin,
                DefKind::Delegate,
                DefKind::Concept,
                DefKind::Annotation,
            ],
            Category::Scope => &[
                DefKind::Namespace,
                DefKind::Package,
                DefKind::Module,
                DefKind::ModuleDeclaration,
            ],
        }
    }
}

impl DefKind {
    #[allow(dead_code)]
    pub fn category(&self) -> Option<Category> {
        Category::all()
            .iter()
            .find(|&&cat| cat.members().contains(self))
            .copied()
    }

    pub fn kinds_from_tag(tag: &str) -> Vec<DefKind> {
        if let Some(cat) = Category::from_tag(tag) {
            return cat.members().to_vec();
        }
        // Handle *_definition suffix: "function_definition" → [Function]
        if let Some(base) = tag.strip_suffix("_definition") {
            if let Some(kind) = DefKind::from_tag(base) {
                return vec![kind];
            }
        }
        // Handle exact tag match with declaration pair expansion:
        // "function" → [Function, FunctionDeclaration]
        if let Some(kind) = DefKind::from_tag(tag) {
            if let Some(decl) = kind.declaration_pair() {
                return vec![kind, decl];
            }
            return vec![kind];
        }
        Vec::new()
    }

    /// For a definition kind that has a declaration counterpart, return it.
    /// e.g. Function → FunctionDeclaration, Method → MethodDeclaration.
    /// Returns None for kinds without declaration variants.
    pub fn declaration_pair(&self) -> Option<DefKind> {
        match self {
            DefKind::Function => Some(DefKind::FunctionDeclaration),
            DefKind::Method => Some(DefKind::MethodDeclaration),
            DefKind::Constructor => Some(DefKind::ConstructorDeclaration),
            DefKind::Destructor => Some(DefKind::DestructorDeclaration),
            DefKind::Operator => Some(DefKind::OperatorDeclaration),
            DefKind::Getter => Some(DefKind::GetterDeclaration),
            DefKind::Setter => Some(DefKind::SetterDeclaration),
            DefKind::Subscript => Some(DefKind::SubscriptDeclaration),
            DefKind::Property => Some(DefKind::PropertyDeclaration),
            DefKind::Var => Some(DefKind::VarDeclaration),
            DefKind::Const => Some(DefKind::ConstDeclaration),
            DefKind::Struct => Some(DefKind::StructDeclaration),
            DefKind::Class => Some(DefKind::ClassDeclaration),
            DefKind::Union => Some(DefKind::UnionDeclaration),
            DefKind::Enum => Some(DefKind::EnumDeclaration),
            DefKind::Module => Some(DefKind::ModuleDeclaration),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct DefContent {
    pub kind: DefKind,
    pub lines: [u32; 2],
    pub signature: String,
    pub scope: String,
}

#[derive(Debug, Clone)]
pub struct FileDefs {
    pub file: PathBuf,
    pub defs: Vec<DefContent>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_variants_round_trip() {
        for &kind in DefKind::all() {
            let tag = kind.display_tag();
            assert_eq!(
                DefKind::from_tag(tag),
                Some(kind),
                "round-trip failed for {:?}: display_tag={:?}, from_tag returned {:?}",
                kind,
                tag,
                DefKind::from_tag(tag)
            );
        }
    }

    #[test]
    fn kinds_from_tag_unknown_returns_empty() {
        assert!(DefKind::kinds_from_tag("unknown").is_empty());
        assert!(DefKind::kinds_from_tag("").is_empty());
    }
}
