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
    Static => "static",
    Namespace => "namespace",
    Package => "package",
    Variant => "variant",
    Destructor => "destructor",
    Subscript => "subscript",
    Annotation => "annotation",
);

// Compile-time assertion: discriminants must be sequential 0..33
const _: () = assert!(
    DefKind::Function as u8 == 0 && DefKind::Static as u8 == 26,
    "DefKind discriminants must be sequential starting from 0"
);

const _: () = assert!(
    DefKind::Package as u8 == 28,
    "DefKind::Package must be discriminant 28"
);

const _: () = assert!(
    DefKind::Annotation as u8 == 32,
    "DefKind::Annotation must be discriminant 32"
);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Category {
    Shape,
    Callable,
    Value,
    Contract,
}

impl Category {
    pub fn display_tag(&self) -> &'static str {
        match self {
            Category::Shape => "shape",
            Category::Callable => "callable",
            Category::Value => "value",
            Category::Contract => "contract",
        }
    }

    pub fn all() -> &'static [Category] {
        &[
            Category::Shape,
            Category::Callable,
            Category::Value,
            Category::Contract,
        ]
    }

    pub fn from_tag(tag: &str) -> Option<Category> {
        match tag {
            "shape" => Some(Category::Shape),
            "callable" => Some(Category::Callable),
            "value" => Some(Category::Value),
            "contract" => Some(Category::Contract),
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
            ],
            Category::Value => &[
                DefKind::Const,
                DefKind::Event,
                DefKind::Field,
                DefKind::Property,
                DefKind::Static,
                DefKind::Variant,
            ],
            Category::Contract => &[
                DefKind::Interface,
                DefKind::Protocol,
                DefKind::Trait,
                DefKind::Extension,
                DefKind::Mixin,
                DefKind::Delegate,
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
        if let Some(kind) = DefKind::from_tag(tag) {
            return vec![kind];
        }
        if let Some(cat) = Category::from_tag(tag) {
            return cat.members().to_vec();
        }
        Vec::new()
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
    fn def_kind_display_tag() {
        assert_eq!(DefKind::Function.display_tag(), "function");
        assert_eq!(DefKind::Class.display_tag(), "class");
        assert_eq!(DefKind::Struct.display_tag(), "struct");
        assert_eq!(DefKind::Method.display_tag(), "method");
        assert_eq!(DefKind::Constructor.display_tag(), "constructor");
        assert_eq!(DefKind::Getter.display_tag(), "getter");
        assert_eq!(DefKind::Setter.display_tag(), "setter");
        assert_eq!(DefKind::Operator.display_tag(), "operator");
        assert_eq!(DefKind::Field.display_tag(), "field");
        assert_eq!(DefKind::Property.display_tag(), "property");
        assert_eq!(DefKind::Static.display_tag(), "static");
        assert_eq!(DefKind::Namespace.display_tag(), "namespace");
        assert_eq!(DefKind::Package.display_tag(), "package");
    }

    #[test]
    fn from_tag_returns_correct_kind() {
        assert_eq!(DefKind::from_tag("function"), Some(DefKind::Function));
        assert_eq!(DefKind::from_tag("interface"), Some(DefKind::Interface));
        assert_eq!(DefKind::from_tag("class"), Some(DefKind::Class));
        assert_eq!(DefKind::from_tag("struct"), Some(DefKind::Struct));
        assert_eq!(DefKind::from_tag("union"), Some(DefKind::Union));
        assert_eq!(DefKind::from_tag("method"), Some(DefKind::Method));
        assert_eq!(DefKind::from_tag("constructor"), Some(DefKind::Constructor));
        assert_eq!(DefKind::from_tag("getter"), Some(DefKind::Getter));
        assert_eq!(DefKind::from_tag("setter"), Some(DefKind::Setter));
        assert_eq!(DefKind::from_tag("operator"), Some(DefKind::Operator));
        assert_eq!(DefKind::from_tag("field"), Some(DefKind::Field));
        assert_eq!(DefKind::from_tag("property"), Some(DefKind::Property));
        assert_eq!(DefKind::from_tag("static"), Some(DefKind::Static));
        assert_eq!(DefKind::from_tag("namespace"), Some(DefKind::Namespace));
        assert_eq!(DefKind::from_tag("package"), Some(DefKind::Package));
    }

    #[test]
    fn from_tag_returns_none_for_unknown() {
        assert_eq!(DefKind::from_tag("func"), None);
        assert_eq!(DefKind::from_tag(""), None);
        assert_eq!(DefKind::from_u8(33), None);
    }

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

    // --- Binary encoding round-trip tests ---

    #[test]
    fn def_kind_binary_round_trip() {
        for &kind in DefKind::all() {
            let encoded = kind.to_u8();
            let decoded = DefKind::from_u8(encoded);
            assert_eq!(
                Some(kind),
                decoded,
                "DefKind binary round-trip failed for {:?}",
                kind
            );
        }
    }

    #[test]
    fn def_kind_from_u8_rejects_unknown() {
        assert_eq!(DefKind::from_u8(255), None);
        assert_eq!(DefKind::from_u8(33), None);
    }

    // --- Category tests ---

    #[test]
    fn category_from_tag() {
        assert_eq!(Category::from_tag("shape"), Some(Category::Shape));
        assert_eq!(Category::from_tag("callable"), Some(Category::Callable));
        assert_eq!(Category::from_tag("value"), Some(Category::Value));
        assert_eq!(Category::from_tag("contract"), Some(Category::Contract));
        assert_eq!(Category::from_tag("function"), None);
        assert_eq!(Category::from_tag("class"), None);
    }

    #[test]
    fn category_display_tag() {
        assert_eq!(Category::Shape.display_tag(), "shape");
        assert_eq!(Category::Callable.display_tag(), "callable");
        assert_eq!(Category::Value.display_tag(), "value");
        assert_eq!(Category::Contract.display_tag(), "contract");
    }

    #[test]
    fn category_members_shape() {
        let members = Category::Shape.members();
        assert!(members.contains(&DefKind::Class));
        assert!(members.contains(&DefKind::Struct));
        assert!(members.contains(&DefKind::Enum));
        assert!(members.contains(&DefKind::Union));
        assert!(members.contains(&DefKind::Record));
        assert!(members.contains(&DefKind::Object));
        assert!(members.contains(&DefKind::Actor));
        assert_eq!(members.len(), 7);
    }

    #[test]
    fn category_members_callable() {
        let members = Category::Callable.members();
        assert!(members.contains(&DefKind::Function));
        assert!(members.contains(&DefKind::Method));
        assert!(members.contains(&DefKind::Constructor));
        assert!(members.contains(&DefKind::Getter));
        assert!(members.contains(&DefKind::Setter));
        assert!(members.contains(&DefKind::Operator));
        assert!(members.contains(&DefKind::Destructor));
        assert!(members.contains(&DefKind::Subscript));
        assert_eq!(members.len(), 8);
    }

    #[test]
    fn category_members_value() {
        let members = Category::Value.members();
        assert!(members.contains(&DefKind::Const));
        assert!(members.contains(&DefKind::Event));
        assert!(members.contains(&DefKind::Field));
        assert!(members.contains(&DefKind::Property));
        assert!(members.contains(&DefKind::Static));
        assert!(members.contains(&DefKind::Variant));
        assert_eq!(members.len(), 6);
    }

    #[test]
    fn category_members_contract() {
        let members = Category::Contract.members();
        assert!(members.contains(&DefKind::Interface));
        assert!(members.contains(&DefKind::Protocol));
        assert!(members.contains(&DefKind::Trait));
        assert!(members.contains(&DefKind::Extension));
        assert!(members.contains(&DefKind::Mixin));
        assert!(members.contains(&DefKind::Delegate));
        assert_eq!(members.len(), 6);
    }

    #[test]
    fn category_all_returns_four() {
        assert_eq!(Category::all().len(), 4);
    }

    #[test]
    fn def_kind_category_mapping() {
        assert_eq!(DefKind::Class.category(), Some(Category::Shape));
        assert_eq!(DefKind::Function.category(), Some(Category::Callable));
        assert_eq!(DefKind::Method.category(), Some(Category::Callable));
        assert_eq!(DefKind::Constructor.category(), Some(Category::Callable));
        assert_eq!(DefKind::Getter.category(), Some(Category::Callable));
        assert_eq!(DefKind::Setter.category(), Some(Category::Callable));
        assert_eq!(DefKind::Operator.category(), Some(Category::Callable));
        assert_eq!(DefKind::Destructor.category(), Some(Category::Callable));
        assert_eq!(DefKind::Subscript.category(), Some(Category::Callable));
        assert_eq!(DefKind::Const.category(), Some(Category::Value));
        assert_eq!(DefKind::Variant.category(), Some(Category::Value));
        assert_eq!(DefKind::Interface.category(), Some(Category::Contract));
        // Standalone kinds have no category
        assert_eq!(DefKind::Alias.category(), None);
        assert_eq!(DefKind::Module.category(), None);
        assert_eq!(DefKind::Macro.category(), None);
        assert_eq!(DefKind::Namespace.category(), None);
        assert_eq!(DefKind::Package.category(), None);
        assert_eq!(DefKind::Annotation.category(), None);
    }

    // --- kinds_from_tag tests ---

    #[test]
    fn kinds_from_tag_single_kind() {
        let kinds = DefKind::kinds_from_tag("function");
        assert_eq!(kinds, vec![DefKind::Function]);
    }

    #[test]
    fn kinds_from_tag_shape_expands() {
        let kinds = DefKind::kinds_from_tag("shape");
        assert_eq!(kinds.len(), 7);
        assert!(kinds.contains(&DefKind::Class));
        assert!(kinds.contains(&DefKind::Struct));
        assert!(kinds.contains(&DefKind::Enum));
        assert!(kinds.contains(&DefKind::Union));
        assert!(kinds.contains(&DefKind::Record));
        assert!(kinds.contains(&DefKind::Object));
        assert!(kinds.contains(&DefKind::Actor));
    }

    #[test]
    fn kinds_from_tag_callable_expands() {
        let kinds = DefKind::kinds_from_tag("callable");
        assert_eq!(kinds.len(), 8);
        assert!(kinds.contains(&DefKind::Function));
        assert!(kinds.contains(&DefKind::Method));
        assert!(kinds.contains(&DefKind::Constructor));
        assert!(kinds.contains(&DefKind::Getter));
        assert!(kinds.contains(&DefKind::Setter));
        assert!(kinds.contains(&DefKind::Operator));
        assert!(kinds.contains(&DefKind::Destructor));
        assert!(kinds.contains(&DefKind::Subscript));
    }

    #[test]
    fn kinds_from_tag_value_expands() {
        let kinds = DefKind::kinds_from_tag("value");
        assert!(kinds.contains(&DefKind::Const));
        assert!(kinds.contains(&DefKind::Event));
        assert!(kinds.contains(&DefKind::Field));
        assert!(kinds.contains(&DefKind::Property));
        assert!(kinds.contains(&DefKind::Static));
        assert!(kinds.contains(&DefKind::Variant));
        assert_eq!(kinds.len(), 6);
    }

    #[test]
    fn kinds_from_tag_contract_expands() {
        let kinds = DefKind::kinds_from_tag("contract");
        assert_eq!(kinds.len(), 6);
    }

    #[test]
    fn kinds_from_tag_unknown_returns_empty() {
        assert!(DefKind::kinds_from_tag("unknown").is_empty());
        assert!(DefKind::kinds_from_tag("").is_empty());
    }
}
