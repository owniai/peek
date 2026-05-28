use std::collections::HashMap;
use std::path::Path;

use crate::model::DefKind;
use crate::parser::LanguageParser;

macro_rules! define_language_registry {
    ($($mod_name:ident: $parser:ident),* $(,)?) => {
        $(use crate::parser::$mod_name::$parser;)*

        pub const KNOWN_LANGUAGES: &[&str] = &[$(crate::parser::$mod_name::LANGUAGE),*];

        const _TOTAL_EXT: usize = 0 $(+ crate::parser::$mod_name::EXTENSIONS.len())*;
        const _ALL_EXT: [&str; _TOTAL_EXT] = {
            let mut arr: [&str; _TOTAL_EXT] = [""; _TOTAL_EXT];
            let mut idx: usize = 0;
            $(
                {
                    let mut i: usize = 0;
                    while i < crate::parser::$mod_name::EXTENSIONS.len() {
                        arr[idx] = crate::parser::$mod_name::EXTENSIONS[i];
                        idx += 1;
                        i += 1;
                    }
                }
            )*
            arr
        };
        #[allow(dead_code)]
        pub const KNOWN_EXTENSIONS: &[&str] = &_ALL_EXT;

        const _TOTAL_ALIAS: usize = 0 $(+ crate::parser::$mod_name::ALIASES.len())*;
        const _ALL_ALIAS: [(&str, &str); _TOTAL_ALIAS] = {
            let mut arr: [(&str, &str); _TOTAL_ALIAS] = [("", ""); _TOTAL_ALIAS];
            let mut idx: usize = 0;
            $(
                {
                    let mut i: usize = 0;
                    while i < crate::parser::$mod_name::ALIASES.len() {
                        arr[idx] = (crate::parser::$mod_name::ALIASES[i], crate::parser::$mod_name::LANGUAGE);
                        idx += 1;
                        i += 1;
                    }
                }
            )*
            arr
        };
        const ALIAS_PAIRS: &[(&str, &str)] = &_ALL_ALIAS;

        impl ParserRegistry {
            pub fn default_registry() -> Self {
                let mut reg = Self::new();
                $(reg.register(Box::new($parser));)*
                reg
            }
        }
    };
}

// Registration order determines default ownership for shared extensions:
// first-registered language wins (e.g., lua owns ".lua"; luau is an alternative).
define_language_registry! {
    python: PythonParser,
    go: GoParser,
    rust: RustParser,
    javascript: JsParser,
    typescript: TsParser,
    java: JavaParser,
    csharp: CSharpParser,
    php: PhpParser,
    c: CParser,
    cpp: CppParser,
    kotlin: KotlinParser,
    swift: SwiftParser,
    ruby: RubyParser,
    dart: DartParser,
    bash: BashParser,
    lua: LuaParser,
    luau: LuauParser,
    objc: ObjCParser,
}

/// Resolve a language name (case-insensitive) to its canonical form.
/// Returns the canonical name if valid, or `None` for unknown languages.
pub fn resolve_language(name: &str) -> Option<&'static str> {
    let lower = name.to_lowercase();
    if let Some(&lang) = KNOWN_LANGUAGES.iter().find(|&&l| l == lower) {
        return Some(lang);
    }
    for &(alias, canonical) in ALIAS_PAIRS {
        if alias == lower {
            return Some(canonical);
        }
    }
    None
}

pub struct ParserRegistry {
    parsers: HashMap<&'static str, Box<dyn LanguageParser>>,
    ext_map: HashMap<&'static str, &'static str>,
    alternatives: HashMap<&'static str, Vec<&'static str>>,
}

impl ParserRegistry {
    pub fn new() -> Self {
        Self {
            parsers: HashMap::new(),
            ext_map: HashMap::new(),
            alternatives: HashMap::new(),
        }
    }

    pub fn register(&mut self, parser: Box<dyn LanguageParser>) {
        let language = parser.language();
        for ext in parser.extensions() {
            if let Some(&existing) = self.ext_map.get(ext) {
                if existing != language {
                    self.alternatives.entry(ext).or_default().push(language);
                }
            } else {
                self.ext_map.insert(ext, language);
            }
        }
        self.parsers.insert(language, parser);
    }

    pub fn get_by_ext(&self, path: &Path, language_hints: &[&str]) -> Option<&dyn LanguageParser> {
        let ext = path.extension()?.to_str()?;
        let default_lang = self.ext_map.get(ext)?;

        // First-registered language owns the extension. Override only when the user
        // explicitly requested a non-default alternative and did not also request the default.
        if let Some(alts) = self.alternatives.get(ext) {
            let resolved: Vec<&str> = language_hints
                .iter()
                .filter_map(|h| resolve_language(h))
                .collect();
            if !resolved.contains(default_lang) {
                if let Some(lang) = alts.iter().find(|&&alt| resolved.contains(&alt)) {
                    return self.parsers.get(*lang).map(|p| p.as_ref());
                }
            }
        }

        self.parsers.get(default_lang).map(|p| p.as_ref())
    }

    pub fn supported_extensions_for_kinds(&self, kinds: &[DefKind]) -> Vec<&str> {
        self.parsers
            .values()
            .filter(|p| p.supported_kinds().iter().any(|k| kinds.contains(k)))
            .flat_map(|p| p.extensions().iter().copied())
            .collect()
    }

    pub fn supported_extensions_for_languages(&self, languages: &[&str]) -> Vec<&str> {
        let canonical: Vec<&str> = languages
            .iter()
            .filter_map(|l| resolve_language(l))
            .collect();
        self.parsers
            .values()
            .filter(|p| canonical.contains(&p.language()))
            .flat_map(|p| p.extensions().iter().copied())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- resolve_language ---

    #[test]
    fn resolve_unknown_returns_none() {
        assert_eq!(resolve_language("unknown"), None);
        assert_eq!(resolve_language("fortran"), None);
        assert_eq!(resolve_language(""), None);
    }

    // --- supported_extensions_for_languages ---

    #[test]
    fn extensions_for_single_language() {
        let reg = ParserRegistry::default_registry();
        let exts = reg.supported_extensions_for_languages(&["rust"]);
        assert!(exts.contains(&"rs"));
        assert!(!exts.contains(&"py"));
    }

    #[test]
    fn extensions_for_multiple_languages() {
        let reg = ParserRegistry::default_registry();
        let exts = reg.supported_extensions_for_languages(&["python", "go"]);
        assert!(exts.contains(&"py"));
        assert!(exts.contains(&"go"));
        assert!(!exts.contains(&"rs"));
    }

    #[test]
    fn extensions_accepts_aliases() {
        let reg = ParserRegistry::default_registry();
        let exts = reg.supported_extensions_for_languages(&["js"]);
        assert!(exts.contains(&"js"));
        assert!(exts.contains(&"jsx"));
        assert!(!exts.contains(&"ts"));
    }

    #[test]
    fn extensions_unknown_language_skipped() {
        let reg = ParserRegistry::default_registry();
        let exts = reg.supported_extensions_for_languages(&["rust", "unknown"]);
        assert!(exts.contains(&"rs"));
    }

    // --- ambiguous extension resolution ---

    #[test]
    fn lua_file_defaults_to_lua_parser() {
        let reg = ParserRegistry::default_registry();
        let parser = reg.get_by_ext(Path::new("foo.lua"), &[]).unwrap();
        assert_eq!(parser.language(), "lua");
    }

    #[test]
    fn lua_file_routed_to_luau_when_language_luau() {
        let reg = ParserRegistry::default_registry();
        let parser = reg.get_by_ext(Path::new("foo.lua"), &["luau"]).unwrap();
        assert_eq!(parser.language(), "luau");
    }

    #[test]
    fn lua_file_stays_lua_when_both_languages_specified() {
        let reg = ParserRegistry::default_registry();
        let parser = reg
            .get_by_ext(Path::new("foo.lua"), &["lua", "luau"])
            .unwrap();
        assert_eq!(parser.language(), "lua");
    }

    #[test]
    fn lua_file_stays_lua_when_language_lua_only() {
        let reg = ParserRegistry::default_registry();
        let parser = reg.get_by_ext(Path::new("foo.lua"), &["lua"]).unwrap();
        assert_eq!(parser.language(), "lua");
    }

    #[test]
    fn luau_file_always_uses_luau_parser() {
        let reg = ParserRegistry::default_registry();
        let parser = reg.get_by_ext(Path::new("foo.luau"), &[]).unwrap();
        assert_eq!(parser.language(), "luau");
        let parser = reg.get_by_ext(Path::new("foo.luau"), &["lua"]).unwrap();
        assert_eq!(parser.language(), "luau");
    }

    #[test]
    fn luau_extensions_included_when_language_luau() {
        let reg = ParserRegistry::default_registry();
        let exts = reg.supported_extensions_for_languages(&["luau"]);
        assert!(exts.contains(&"luau"), "should contain .luau");
        assert!(exts.contains(&"lua"), "should contain .lua for luau");
    }

    // --- .h extension alternatives (cpp / objc) ---

    #[test]
    fn h_file_defaults_to_cplusplus_parser() {
        let reg = ParserRegistry::default_registry();
        let parser = reg.get_by_ext(Path::new("foo.h"), &[]).unwrap();
        assert_eq!(parser.language(), "cplusplus");
    }

    #[test]
    fn h_file_routed_to_objc_when_language_objc() {
        let reg = ParserRegistry::default_registry();
        let parser = reg.get_by_ext(Path::new("foo.h"), &["objc"]).unwrap();
        assert_eq!(parser.language(), "objc");
    }

    #[test]
    fn h_file_stays_cplusplus_when_both_cpp_and_objc_specified() {
        let reg = ParserRegistry::default_registry();
        let parser = reg
            .get_by_ext(Path::new("foo.h"), &["cpp", "objc"])
            .unwrap();
        assert_eq!(parser.language(), "cplusplus");
    }
}
