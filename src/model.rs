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
    fn def_content_fields() {
        let def = DefContent {
            kind: DefKind::Function,
            lines: [15, 30],
            signature: "def process();".to_string(),
            scope: "process".to_string(),
        };
        assert_eq!(def.kind, DefKind::Function);
        assert_eq!(def.kind.display_tag(), "function");
        assert_eq!(def.lines, [15, 30]);
    }

    #[test]
    fn def_content_for_class() {
        let def = DefContent {
            kind: DefKind::Class,
            lines: [42, 85],
            signature: "class MyClass(Base)".to_string(),
            scope: "MyClass".to_string(),
        };
        assert_eq!(def.kind.display_tag(), "class");
    }

    #[test]
    fn def_content_for_struct() {
        let def = DefContent {
            kind: DefKind::Struct,
            lines: [1, 10],
            signature: "struct Config {".to_string(),
            scope: "Config".to_string(),
        };
        assert_eq!(def.kind.display_tag(), "struct");
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
    fn new_variants_display_tag() {
        assert_eq!(DefKind::Record.display_tag(), "record");
        assert_eq!(DefKind::Delegate.display_tag(), "delegate");
        assert_eq!(DefKind::Event.display_tag(), "event");
    }

    #[test]
    fn new_variants_from_tag_round_trip() {
        assert_eq!(DefKind::from_tag("record"), Some(DefKind::Record));
        assert_eq!(DefKind::from_tag("delegate"), Some(DefKind::Delegate));
        assert_eq!(DefKind::from_tag("event"), Some(DefKind::Event));
    }

    #[test]
    fn new_variants_in_all() {
        let all = DefKind::all();
        assert!(all.contains(&DefKind::Record));
        assert!(all.contains(&DefKind::Delegate));
        assert!(all.contains(&DefKind::Event));
    }

    #[test]
    fn new_variants_unknown_tag_still_none() {
        assert_eq!(DefKind::from_tag("recordstruct"), None);
        assert_eq!(DefKind::from_tag("del"), None);
        assert_eq!(DefKind::from_tag("evt"), None);
    }

    #[test]
    fn all_count_is_eighteen() {
        assert_eq!(DefKind::all().len(), 18);
    }

    #[test]
    fn dart_mixin_display_tag() {
        assert_eq!(DefKind::Mixin.display_tag(), "mixin");
    }

    #[test]
    fn dart_mixin_from_tag_round_trip() {
        assert_eq!(DefKind::from_tag("mixin"), Some(DefKind::Mixin));
    }

    #[test]
    fn dart_mixin_in_all() {
        assert!(DefKind::all().contains(&DefKind::Mixin));
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

    #[test]
    fn swift_variants_display_tag() {
        assert_eq!(DefKind::Protocol.display_tag(), "protocol");
        assert_eq!(DefKind::Actor.display_tag(), "actor");
        assert_eq!(DefKind::Extension.display_tag(), "extension");
    }

    #[test]
    fn swift_variants_from_tag_round_trip() {
        assert_eq!(DefKind::from_tag("protocol"), Some(DefKind::Protocol));
        assert_eq!(DefKind::from_tag("actor"), Some(DefKind::Actor));
        assert_eq!(DefKind::from_tag("extension"), Some(DefKind::Extension));
    }

    #[test]
    fn swift_variants_in_all() {
        let all = DefKind::all();
        assert!(all.contains(&DefKind::Protocol));
        assert!(all.contains(&DefKind::Actor));
        assert!(all.contains(&DefKind::Extension));
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
