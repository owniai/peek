use std::sync::LazyLock;

use anyhow::{Result, bail};
use regex::Regex;
use regex_syntax::hir::{Class, ClassUnicode, Hir, HirKind, Look};

fn is_smart_case(s: &str) -> bool {
    !s.chars().any(|c| c.is_uppercase())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaseSensitivity {
    Sensitive,
    Insensitive,
    SmartCase,
}

#[derive(Debug, Clone)]
pub enum MatchMode {
    Regex { compiled: Regex },
    All,
}

fn is_regex_meta(c: char) -> bool {
    matches!(
        c,
        '.' | '*' | '+' | '?' | '|' | '(' | ')' | '[' | ']' | '{' | '}' | '\\' | '^' | '$'
    )
}

fn compile_regex(pattern: &str, case_insensitive: bool, word: bool) -> Result<Regex> {
    let wrapped = match (case_insensitive, word) {
        (true, true) => format!(r"(?i)\b{{start-half}}(?:{})\b{{end-half}}", pattern),
        (false, true) => format!(r"\b{{start-half}}(?:{})\b{{end-half}}", pattern),
        (true, false) => format!("(?i)(?:{})", pattern),
        (false, false) => format!("(?:{})", pattern),
    };
    Regex::new(&wrapped).map_err(regex_err)
}

fn regex_err(e: impl std::fmt::Display) -> anyhow::Error {
    anyhow::anyhow!("invalid regex pattern: {}", e)
}

// ===== HIR Analysis =====

static WORD_CLASS: LazyLock<ClassUnicode> = LazyLock::new(|| {
    let hir = regex_syntax::ParserBuilder::new()
        .build()
        .parse("\\w")
        .expect("hardcoded \\w pattern is valid");
    match hir.kind() {
        HirKind::Class(Class::Unicode(uc)) => uc.clone(),
        _ => unreachable!("\\w must produce a Unicode class"),
    }
});

fn analyze_pattern(input: &str) -> Result<HirResult> {
    let hir = regex_syntax::ParserBuilder::new()
        .build()
        .parse(input)
        .map_err(regex_err)?;
    Ok(analyze_hir(&hir, &WORD_CLASS, true))
}

#[derive(Clone, Copy, PartialEq)]
enum HirResult {
    Impossible,
    MatchesAll,
    Normal,
}

fn analyze_hir(hir: &Hir, word_class: &ClassUnicode, is_top: bool) -> HirResult {
    match hir.kind() {
        HirKind::Empty => HirResult::Normal,

        HirKind::Literal(_) => HirResult::Normal,

        HirKind::Class(class) => analyze_class(class, word_class),

        HirKind::Look(look) => analyze_look(*look, is_top),

        HirKind::Repetition(rep) => {
            let sub = analyze_hir(&rep.sub, word_class, false);
            if sub == HirResult::Impossible && rep.min > 0 {
                HirResult::Impossible
            } else if sub == HirResult::Impossible {
                HirResult::Normal
            } else if is_top && sub == HirResult::MatchesAll && rep.min == 0 && rep.max.is_none() {
                HirResult::MatchesAll
            } else {
                sub
            }
        }

        HirKind::Capture(cap) => {
            let sub = analyze_hir(&cap.sub, word_class, is_top);
            if is_top && sub == HirResult::MatchesAll {
                HirResult::MatchesAll
            } else {
                sub
            }
        }

        HirKind::Concat(subs) => {
            let mut all_match_eligible = is_top && !subs.is_empty();
            for sub in subs {
                let result = analyze_hir(sub, word_class, false);
                if result == HirResult::Impossible && is_required_element(sub) {
                    return HirResult::Impossible;
                }
                if all_match_eligible {
                    match sub.kind() {
                        HirKind::Look(_) => {}
                        HirKind::Repetition(rep) => {
                            if result != HirResult::MatchesAll || rep.min != 0 || rep.max.is_some()
                            {
                                all_match_eligible = false;
                            }
                        }
                        HirKind::Capture(_) => {
                            if result != HirResult::MatchesAll {
                                all_match_eligible = false;
                            }
                        }
                        _ => all_match_eligible = false,
                    }
                }
            }
            if all_match_eligible {
                HirResult::MatchesAll
            } else {
                HirResult::Normal
            }
        }

        HirKind::Alternation(subs) => {
            let mut any_possible = false;
            let mut all_match = true;
            for sub in subs {
                let result = analyze_hir(sub, word_class, is_top);
                match result {
                    HirResult::Impossible => {}
                    HirResult::Normal => {
                        any_possible = true;
                        all_match = false;
                    }
                    HirResult::MatchesAll => {
                        any_possible = true;
                    }
                }
            }
            if !any_possible {
                HirResult::Impossible
            } else if all_match && is_top {
                HirResult::MatchesAll
            } else {
                HirResult::Normal
            }
        }
    }
}

fn analyze_class(class: &Class, word_class: &ClassUnicode) -> HirResult {
    match class {
        Class::Unicode(uc) => {
            let mut intersected = uc.clone();
            intersected.intersect(word_class);
            if intersected.ranges().is_empty() {
                HirResult::Impossible
            } else if intersected.ranges() == word_class.ranges() {
                HirResult::MatchesAll
            } else {
                HirResult::Normal
            }
        }
        Class::Bytes(bc) => {
            const WORD_RANGES: &[(u8, u8)] =
                &[(b'0', b'9'), (b'A', b'Z'), (b'_', b'_'), (b'a', b'z')];
            let has_word = bc.iter().any(|r| {
                WORD_RANGES
                    .iter()
                    .any(|&(ws, we)| r.start() <= we && r.end() >= ws)
            });
            if !has_word {
                HirResult::Impossible
            } else {
                HirResult::Normal
            }
        }
    }
}

fn analyze_look(look: Look, is_top: bool) -> HirResult {
    match look {
        Look::Start | Look::StartLF | Look::End | Look::EndLF => HirResult::Normal,
        Look::WordAscii | Look::WordAsciiNegate | Look::WordUnicode | Look::WordUnicodeNegate => {
            if is_top {
                HirResult::Normal
            } else {
                HirResult::Impossible
            }
        }
        Look::WordStartAscii
        | Look::WordEndAscii
        | Look::WordStartHalfAscii
        | Look::WordEndHalfAscii
        | Look::WordStartUnicode
        | Look::WordEndUnicode
        | Look::WordStartHalfUnicode
        | Look::WordEndHalfUnicode => HirResult::Normal,
        Look::StartCRLF | Look::EndCRLF => HirResult::Normal,
    }
}

fn is_required_element(hir: &Hir) -> bool {
    match hir.kind() {
        HirKind::Repetition(rep) => rep.min > 0,
        HirKind::Look(_) => false,
        _ => true,
    }
}

// ===== MatchMode impl =====

impl MatchMode {
    pub fn from_user_input(name: &str, case_insensitive: bool, word: bool) -> Result<MatchMode> {
        let has_meta = name.contains(is_regex_meta);

        let escaped = if !has_meta && word {
            regex::escape(name)
        } else {
            name.to_string()
        };

        match analyze_pattern(name)? {
            HirResult::Impossible => {
                bail!("error: pattern cannot match any identifier (impossible character class)")
            }
            HirResult::MatchesAll => {
                if word {
                    let compiled = compile_regex(&escaped, case_insensitive, true)?;
                    Ok(MatchMode::Regex { compiled })
                } else {
                    Ok(MatchMode::All)
                }
            }
            HirResult::Normal => {
                let compiled = compile_regex(&escaped, case_insensitive, word)?;
                Ok(MatchMode::Regex { compiled })
            }
        }
    }

    pub fn matches_ident(&self, ident: &str) -> bool {
        match self {
            MatchMode::Regex { compiled, .. } => compiled.is_match(ident),
            MatchMode::All => true,
        }
    }
}

// ===== ParsedPattern =====

pub struct ParsedPattern {
    mode: MatchMode,
    original: String,
}

impl ParsedPattern {
    pub fn parse(input: &str, case: CaseSensitivity, word: bool) -> Result<Self> {
        let case_insensitive = match case {
            CaseSensitivity::Sensitive => false,
            CaseSensitivity::Insensitive => true,
            CaseSensitivity::SmartCase => is_smart_case(input),
        };

        let mode = MatchMode::from_user_input(input, case_insensitive, word)?;
        Ok(ParsedPattern {
            mode,
            original: input.to_string(),
        })
    }

    pub fn mode(&self) -> &MatchMode {
        &self.mode
    }

    pub fn display_name(&self) -> &str {
        &self.original
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ===== Exact mode: substring matching =====

    #[test]
    fn exact_substring_matches_self() {
        let mode = MatchMode::from_user_input("foo", false, false).unwrap();
        assert!(mode.matches_ident("foo"));
    }

    #[test]
    fn exact_substring_matches_prefix() {
        let mode = MatchMode::from_user_input("foo", false, false).unwrap();
        assert!(mode.matches_ident("foobar"));
    }

    #[test]
    fn exact_substring_matches_suffix() {
        let mode = MatchMode::from_user_input("foo", false, false).unwrap();
        assert!(mode.matches_ident("afoo"));
    }

    #[test]
    fn exact_substring_matches_middle() {
        let mode = MatchMode::from_user_input("foo", false, false).unwrap();
        assert!(mode.matches_ident("xfooy"));
    }

    #[test]
    fn exact_substring_no_match() {
        let mode = MatchMode::from_user_input("foo", false, false).unwrap();
        assert!(!mode.matches_ident("fobar"));
    }

    #[test]
    fn exact_substring_case_insensitive() {
        let mode = MatchMode::from_user_input("foo", true, false).unwrap();
        assert!(mode.matches_ident("FooBar"));
        assert!(mode.matches_ident("AFOOBAR"));
        assert!(mode.matches_ident("foobar"));
        assert!(!mode.matches_ident("fobar"));
    }

    // ===== Fuzzy trigger: regex metacharacters =====

    #[test]
    fn regex_metachar_matching_behavior() {
        // Regex metachar patterns match identifiers as substrings
        let mode = MatchMode::from_user_input("get+", false, false).unwrap();
        assert!(mode.matches_ident("getter"));
        assert!(mode.matches_ident("mygetter"));
        assert!(!mode.matches_ident("foo"));

        let mode = MatchMode::from_user_input("get?", false, false).unwrap();
        assert!(mode.matches_ident("ge"));
        assert!(mode.matches_ident("get"));
        assert!(mode.matches_ident("getter"));
    }

    // ===== Fuzzy: substring regex matching =====

    #[test]
    fn fuzzy_alternation_matches_ident() {
        let mode = MatchMode::from_user_input("process|handle", false, false).unwrap();
        assert!(mode.matches_ident("process"));
        assert!(mode.matches_ident("handle"));
        assert!(mode.matches_ident("myprocess"));
        assert!(mode.matches_ident("handler"));
        assert!(!mode.matches_ident("foo"));
    }

    #[test]
    fn fuzzy_dot_star_matches_ident() {
        let mode = MatchMode::from_user_input("My.*er", false, false).unwrap();
        assert!(mode.matches_ident("MyHandler"));
        assert!(mode.matches_ident("MyHandlerFactory"));
        assert!(!mode.matches_ident("MyBar"));
    }

    #[test]
    fn fuzzy_group_pattern_matches_ident() {
        let mode = MatchMode::from_user_input("(process|handle)_data", false, false).unwrap();
        assert!(mode.matches_ident("process_data"));
        assert!(mode.matches_ident("handle_data"));
        assert!(mode.matches_ident("myprocess_data"));
        assert!(!mode.matches_ident("process_event"));
    }

    // ===== Word boundary matching (-w) =====

    #[test]
    fn word_matches_exact_ident() {
        let mode = MatchMode::from_user_input("foo", false, true).unwrap();
        assert!(mode.matches_ident("foo"));
    }

    #[test]
    fn word_no_match_substring() {
        let mode = MatchMode::from_user_input("foo", false, true).unwrap();
        assert!(!mode.matches_ident("foobar"));
        assert!(!mode.matches_ident("afoo"));
    }

    #[test]
    fn word_matches_scope_with_separator() {
        let mode = MatchMode::from_user_input("foo", false, true).unwrap();
        assert!(mode.matches_ident("MyClass::foo"));
        assert!(mode.matches_ident("myclass.foo"));
    }

    #[test]
    fn word_matches_class_in_scope() {
        let mode = MatchMode::from_user_input("MyClass", false, true).unwrap();
        assert!(mode.matches_ident("MyClass::myfunc"));
    }

    #[test]
    fn word_case_insensitive() {
        let mode = MatchMode::from_user_input("foo", true, true).unwrap();
        assert!(mode.matches_ident("Foo"));
        assert!(mode.matches_ident("FOO"));
        assert!(mode.matches_ident("MyClass::FOO"));
        assert!(!mode.matches_ident("foobar"));
    }

    #[test]
    fn word_with_regex() {
        let mode = MatchMode::from_user_input("get.*", false, true).unwrap();
        assert!(mode.matches_ident("get"));
        assert!(mode.matches_ident("get_data"));
        assert!(!mode.matches_ident("myget"));
        assert!(mode.matches_ident("MyClass::get"));
    }

    // ===== HIR Impossible detection =====

    #[test]
    fn reject_pure_whitespace_as_impossible() {
        let result = MatchMode::from_user_input("\\s+", false, false);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .to_lowercase()
                .contains("impossible")
        );
    }

    #[test]
    fn reject_non_word_char_as_impossible() {
        let result = MatchMode::from_user_input("\\W", false, false);
        assert!(result.is_err());
    }

    #[test]
    fn accept_alternation_with_valid_branch() {
        let mode = MatchMode::from_user_input("\\s|foo", false, false).unwrap();
        assert!(mode.matches_ident("foo"));
    }

    // ===== HIR All-match detection =====

    #[test]
    fn non_all_patterns_match_selectively() {
        // Literal patterns are not All — they match only identifiers containing the pattern
        let mode = MatchMode::from_user_input("foo", false, false).unwrap();
        assert!(mode.matches_ident("foo"));
        assert!(mode.matches_ident("foobar"));
        assert!(!mode.matches_ident("bar"));

        // Regex patterns with literals are not All — they match selectively
        let mode = MatchMode::from_user_input("foo.*bar", false, false).unwrap();
        assert!(mode.matches_ident("foobar"));
        assert!(mode.matches_ident("fooXbar"));
        assert!(!mode.matches_ident("baz"));
    }

    // ===== Invalid regex =====

    #[test]
    fn reject_invalid_regex() {
        let result = MatchMode::from_user_input("(ab", false, false);
        assert!(result.is_err());
    }

    #[test]
    fn reject_invalid_regex_close_paren() {
        let result = MatchMode::from_user_input("ab)", false, false);
        assert!(result.is_err());
    }

    // ===== MatchMode::All =====

    #[test]
    fn all_matches_any_ident() {
        let mode = MatchMode::All;
        assert!(mode.matches_ident("anything"));
        assert!(mode.matches_ident(""));
        assert!(mode.matches_ident("MyClass"));
    }

    // ===== Case sensitivity =====

    #[test]
    fn exact_default_case_sensitive() {
        let mode = MatchMode::from_user_input("foo", false, false).unwrap();
        assert!(mode.matches_ident("foo"));
        assert!(mode.matches_ident("foobar"));
        assert!(!mode.matches_ident("Foo"));
    }

    #[test]
    fn exact_ignore_case() {
        let mode = MatchMode::from_user_input("foo", true, false).unwrap();
        assert!(mode.matches_ident("foo"));
        assert!(mode.matches_ident("Foo"));
        assert!(mode.matches_ident("FOObar"));
    }

    // ===== ParsedPattern integration =====

    #[test]
    fn parse_literal_compiled_as_regex() {
        let parsed =
            ParsedPattern::parse("simple_name", CaseSensitivity::Sensitive, false).unwrap();
        assert!(matches!(parsed.mode(), MatchMode::Regex { .. }));
        assert!(parsed.mode().matches_ident("simple_name"));
        assert!(parsed.mode().matches_ident("my_simple_name"));
        assert!(!parsed.mode().matches_ident("Simple_Name"));
    }

    // ===== CaseSensitivity with ParsedPattern =====

    #[test]
    fn parsed_pattern_sensitive_exact() {
        let parsed = ParsedPattern::parse("foo", CaseSensitivity::Sensitive, false).unwrap();
        assert!(parsed.mode().matches_ident("foo"));
        assert!(!parsed.mode().matches_ident("Foo"));
    }

    #[test]
    fn parsed_pattern_insensitive_exact() {
        let parsed = ParsedPattern::parse("foo", CaseSensitivity::Insensitive, false).unwrap();
        assert!(parsed.mode().matches_ident("foo"));
        assert!(parsed.mode().matches_ident("Foo"));
    }

    #[test]
    fn parsed_pattern_smart_case_lowercase() {
        let parsed = ParsedPattern::parse("foo", CaseSensitivity::SmartCase, false).unwrap();
        assert!(parsed.mode().matches_ident("foo"));
        assert!(parsed.mode().matches_ident("Foo"));
    }

    #[test]
    fn parsed_pattern_smart_case_uppercase() {
        let parsed = ParsedPattern::parse("Foo", CaseSensitivity::SmartCase, false).unwrap();
        assert!(parsed.mode().matches_ident("Foo"));
        assert!(!parsed.mode().matches_ident("foo"));
    }

    // ===== Word flag with ParsedPattern =====

    #[test]
    fn parsed_pattern_word_exact() {
        let parsed = ParsedPattern::parse("foo", CaseSensitivity::Sensitive, true).unwrap();
        assert!(parsed.mode().matches_ident("foo"));
        assert!(!parsed.mode().matches_ident("foobar"));
    }

    #[test]
    fn parsed_pattern_word_case_insensitive() {
        let parsed = ParsedPattern::parse("foo", CaseSensitivity::Insensitive, true).unwrap();
        assert!(parsed.mode().matches_ident("FOO"));
        assert!(!parsed.mode().matches_ident("foobar"));
    }

    // ===== Word boundary with non-word prefix (align with ripgrep half-boundary) =====

    #[test]
    fn word_matches_dollar_prefix_toplevel() {
        let mode = MatchMode::from_user_input("\\$ZodString", false, true).unwrap();
        assert!(mode.matches_ident("$ZodString"));
    }

    #[test]
    fn word_matches_dollar_prefix_in_scope() {
        let mode = MatchMode::from_user_input("\\$ZodString", false, true).unwrap();
        assert!(mode.matches_ident("MyClass.$ZodString"));
        assert!(mode.matches_ident("MyClass::$ZodString"));
    }

    #[test]
    fn word_no_match_dollar_prefix_after_word_char() {
        let mode = MatchMode::from_user_input("\\$ZodString", false, true).unwrap();
        assert!(!mode.matches_ident("X$ZodString"));
    }
}
