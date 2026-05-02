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
    Type => "type",
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
);

// Compile-time assertion: discriminants must be sequential 0..17
const _: () = assert!(
    DefKind::Function as u8 == 0 && DefKind::Macro as u8 == 17,
    "DefKind discriminants must be sequential starting from 0"
);

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
    }

    #[test]
    fn from_tag_returns_correct_kind() {
        assert_eq!(DefKind::from_tag("function"), Some(DefKind::Function));
        assert_eq!(DefKind::from_tag("interface"), Some(DefKind::Interface));
        assert_eq!(DefKind::from_tag("class"), Some(DefKind::Class));
        assert_eq!(DefKind::from_tag("struct"), Some(DefKind::Struct));
    }

    #[test]
    fn from_tag_returns_none_for_unknown() {
        assert_eq!(DefKind::from_tag("func"), None);
        assert_eq!(DefKind::from_tag(""), None);
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
        assert_eq!(DefKind::from_u8(18), None);
    }
}
