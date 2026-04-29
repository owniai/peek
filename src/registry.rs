use std::collections::HashMap;
use std::path::Path;

use crate::model::DefKind;
use crate::parser::LanguageParser;
use crate::parser::bash::BashParser;
use crate::parser::c::CParser;
use crate::parser::cpp::CppParser;
use crate::parser::csharp::CSharpParser;
use crate::parser::dart::DartParser;
use crate::parser::go::GoParser;
use crate::parser::java::JavaParser;
use crate::parser::javascript::JsParser;
use crate::parser::kotlin::KotlinParser;
use crate::parser::lua::LuaParser;
use crate::parser::php::PhpParser;
use crate::parser::python::PythonParser;
use crate::parser::ruby::RubyParser;
use crate::parser::rust::RustParser;
use crate::parser::swift::SwiftParser;
use crate::parser::typescript::TsParser;

pub struct ParserRegistry {
    parsers: HashMap<&'static str, Box<dyn LanguageParser>>,
    ext_map: HashMap<&'static str, &'static str>,
}

impl ParserRegistry {
    pub fn new() -> Self {
        Self {
            parsers: HashMap::new(),
            ext_map: HashMap::new(),
        }
    }

    pub fn register(&mut self, parser: Box<dyn LanguageParser>) {
        let language = parser.language();
        for ext in parser.extensions() {
            let key = ext.strip_prefix('.').unwrap_or(ext);
            self.ext_map.insert(key, language);
        }
        self.parsers.insert(language, parser);
    }

    pub fn get_by_ext(&self, path: &Path) -> Option<&dyn LanguageParser> {
        let ext = path.extension()?.to_str()?;
        let language = self.ext_map.get(ext)?;
        self.parsers.get(language).map(|p| p.as_ref())
    }

    pub fn supported_extensions_for_kinds(&self, kinds: &[DefKind]) -> Vec<&str> {
        self.parsers
            .values()
            .filter(|p| p.supported_kinds().iter().any(|k| kinds.contains(k)))
            .flat_map(|p| p.extensions())
            .map(|ext| ext.strip_prefix('.').unwrap_or(ext))
            .collect()
    }

    pub fn default_registry() -> Self {
        let mut reg = Self::new();
        reg.register(Box::new(GoParser));
        reg.register(Box::new(PythonParser));
        reg.register(Box::new(RustParser));
        reg.register(Box::new(JsParser));
        reg.register(Box::new(TsParser));
        reg.register(Box::new(JavaParser));
        reg.register(Box::new(CSharpParser));
        reg.register(Box::new(PhpParser));
        reg.register(Box::new(CParser));
        reg.register(Box::new(CppParser));
        reg.register(Box::new(KotlinParser));
        reg.register(Box::new(SwiftParser));
        reg.register(Box::new(RubyParser));
        reg.register(Box::new(DartParser));
        reg.register(Box::new(BashParser));
        reg.register(Box::new(LuaParser));
        reg
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{DefContent, DefKind};
    use crate::parser::{LanguageParser, MatchMode};
    use std::path::PathBuf;

    struct AlphaParser;
    impl LanguageParser for AlphaParser {
        fn language(&self) -> &'static str {
            "alpha"
        }
        fn extensions(&self) -> &'static [&'static str] {
            &[".a", ".alpha"]
        }
        fn supported_kinds(&self) -> &'static [DefKind] {
            &[DefKind::Function, DefKind::Class]
        }
        fn init_parser(&self) -> tree_sitter::Parser {
            tree_sitter::Parser::new()
        }
        fn extract_with(
            &self,
            _mode: &MatchMode,
            _kinds: &[DefKind],
            _source: &str,
            _parser: &mut tree_sitter::Parser,
        ) -> Result<Vec<DefContent>, ()> {
            Ok(vec![])
        }
    }

    struct BetaParser;
    impl LanguageParser for BetaParser {
        fn language(&self) -> &'static str {
            "beta"
        }
        fn extensions(&self) -> &'static [&'static str] {
            &[".b"]
        }
        fn supported_kinds(&self) -> &'static [DefKind] {
            &[DefKind::Function, DefKind::Struct]
        }
        fn init_parser(&self) -> tree_sitter::Parser {
            tree_sitter::Parser::new()
        }
        fn extract_with(
            &self,
            _mode: &MatchMode,
            _kinds: &[DefKind],
            _source: &str,
            _parser: &mut tree_sitter::Parser,
        ) -> Result<Vec<DefContent>, ()> {
            Ok(vec![])
        }
    }

    #[test]
    fn register_and_lookup_by_extension() {
        let mut reg = ParserRegistry::new();
        reg.register(Box::new(AlphaParser));
        reg.register(Box::new(BetaParser));

        assert_eq!(
            reg.get_by_ext(&PathBuf::from("file.a")).unwrap().language(),
            "alpha"
        );
        assert_eq!(
            reg.get_by_ext(&PathBuf::from("file.alpha"))
                .unwrap()
                .language(),
            "alpha"
        );
        assert_eq!(
            reg.get_by_ext(&PathBuf::from("file.b")).unwrap().language(),
            "beta"
        );
    }

    #[test]
    fn unknown_extension_returns_none() {
        let reg = ParserRegistry::new();
        assert!(reg.get_by_ext(&PathBuf::from("file.unknown")).is_none());
    }

    #[test]
    fn empty_registry_returns_none() {
        let reg = ParserRegistry::new();
        assert!(reg.get_by_ext(&PathBuf::from("file.py")).is_none());
    }
}
