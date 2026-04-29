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

#[derive(Debug)]
pub enum MatchMode {
    Exact {
        name: String,
        case_insensitive: bool,
    },
    Fuzzy {
        compiled: Regex,
    },
    All,
}

fn is_regex_meta(c: char) -> bool {
    matches!(
        c,
        '.' | '*' | '+' | '?' | '|' | '(' | ')' | '[' | ']' | '{' | '}' | '\\' | '^' | '$'
    )
}

fn compile_anchored_regex(pattern: &str, case_insensitive: bool) -> Result<Regex> {
    let anchored = if case_insensitive {
        format!("(?i)^(?:{})$", pattern)
    } else {
        format!("^(?:{})$", pattern)
    };
    Regex::new(&anchored).map_err(regex_err)
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

        HirKind::Literal(lit) => {
            let Ok(s) = std::str::from_utf8(&lit.0) else {
                return HirResult::Impossible;
            };
            for c in s.chars() {
                if !is_word_char(c, word_class) {
                    return HirResult::Impossible;
                }
            }
            HirResult::Normal
        }

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
            // Single pass: check for Impossible and track all-match eligibility
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
                // uc is a superset of \w (e.g., `.` which is any char, or `\w` itself)
                HirResult::MatchesAll
            } else {
                HirResult::Normal
            }
        }
        Class::Bytes(bc) => {
            // Check overlap with ASCII word bytes using range comparison
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
                // At top level in a concat, \b doesn't block but also doesn't help
                HirResult::Normal
            } else {
                // \b inside a pattern (e.g., between word chars) is impossible
                // for matching pure \w+ identifiers
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

fn is_word_char(c: char, word_class: &ClassUnicode) -> bool {
    word_class
        .ranges()
        .iter()
        .any(|r| r.start() <= c && c <= r.end())
}

/// Check if a HIR element is "required" (must match at least once).
fn is_required_element(hir: &Hir) -> bool {
    match hir.kind() {
        HirKind::Repetition(rep) => rep.min > 0,
        HirKind::Look(_) => false,
        _ => true,
    }
}

// ===== MatchMode impl =====

impl MatchMode {
    pub fn from_user_input(name: &str, case_insensitive: bool) -> Result<MatchMode> {
        if !name.contains(is_regex_meta) {
            return Ok(MatchMode::Exact {
                name: name.to_string(),
                case_insensitive,
            });
        }

        match analyze_pattern(name)? {
            HirResult::Impossible => {
                bail!("error: pattern cannot match any identifier (impossible character class)")
            }
            HirResult::MatchesAll => Ok(MatchMode::All),
            HirResult::Normal => {
                let compiled = compile_anchored_regex(name, case_insensitive)?;
                Ok(MatchMode::Fuzzy { compiled })
            }
        }
    }

    pub fn matches_ident(&self, ident: &str) -> bool {
        match self {
            MatchMode::Exact {
                name,
                case_insensitive,
            } => {
                if *case_insensitive {
                    ident.eq_ignore_ascii_case(name)
                } else {
                    ident == name
                }
            }
            MatchMode::Fuzzy { compiled, .. } => compiled.is_match(ident),
            MatchMode::All => true,
        }
    }
}

// ===== ScopeFilter =====

pub struct ScopeFilter {
    scope_regex: Regex,
    separator: &'static str,
}

impl ScopeFilter {
    pub(crate) fn new(
        scope_str: &str,
        separator: &'static str,
        case_insensitive: bool,
    ) -> Result<Self> {
        let regex_str = if separator == "\\" {
            // Unescape \\ → \ first (for new double-backslash syntax), then regex-escape
            let unescaped = scope_str.replace("\\\\", "\\");
            regex::escape(&unescaped)
        } else {
            scope_str.to_string()
        };
        let compiled = compile_anchored_regex(&regex_str, case_insensitive)?;
        Ok(ScopeFilter {
            scope_regex: compiled,
            separator,
        })
    }

    pub fn matches_scope(&self, scope: &str) -> bool {
        let prefix = extract_scope_prefix(scope, self.separator);
        self.scope_regex.is_match(prefix)
    }

    pub fn separator(&self) -> &'static str {
        self.separator
    }
}

fn extract_scope_prefix<'a>(scope: &'a str, separator: &str) -> &'a str {
    match scope.rfind(separator) {
        Some(idx) => &scope[..idx + separator.len()],
        None => "",
    }
}

// ===== ParsedPattern =====

pub struct ParsedPattern {
    mode: MatchMode,
    scope_filter: Option<ScopeFilter>,
    original: String,
}

impl ParsedPattern {
    pub fn parse(input: &str, case: CaseSensitivity) -> Result<Self> {
        if input == "..." {
            return Ok(ParsedPattern {
                mode: MatchMode::All,
                scope_filter: None,
                original: input.to_string(),
            });
        }

        let case_insensitive = match case {
            CaseSensitivity::Sensitive => false,
            CaseSensitivity::Insensitive => true,
            CaseSensitivity::SmartCase => is_smart_case(input),
        };

        if let Some((name_part, scope_str, separator)) = detect_scope_separator(input) {
            if name_part.is_empty() {
                bail!("error: empty name after scope separator");
            }
            let mode = MatchMode::from_user_input(name_part, case_insensitive)?;
            let scope_filter = ScopeFilter::new(scope_str, separator, case_insensitive)?;
            Ok(ParsedPattern {
                mode,
                scope_filter: Some(scope_filter),
                original: input.to_string(),
            })
        } else {
            let mode = MatchMode::from_user_input(input, case_insensitive)?;
            Ok(ParsedPattern {
                mode,
                scope_filter: None,
                original: input.to_string(),
            })
        }
    }

    pub fn mode(&self) -> &MatchMode {
        &self.mode
    }

    pub fn scope_filter(&self) -> Option<&ScopeFilter> {
        self.scope_filter.as_ref()
    }

    pub fn display_name(&self) -> &str {
        &self.original
    }
}

/// Detect scope separator and split the input pattern.
///
/// Priority: `::` > `\.` > `\\` > `\` + letter.
fn detect_scope_separator(input: &str) -> Option<(&str, &str, &'static str)> {
    // Priority 1: :: separator
    if let Some(idx) = input.rfind("::") {
        let name = &input[idx + 2..];
        let scope = &input[..idx + 2];
        return Some((name, scope, "::"));
    }

    // Priority 2: \. (escaped dot = . separator)
    if let Some(pos) = find_escaped_dot(input) {
        let name = &input[pos + 2..];
        let scope = &input[..pos + 2];
        return Some((name, scope, "."));
    }

    // Priority 3a: \\ (double backslash = \ separator, new syntax)
    let bytes = input.as_bytes();
    if let Some(i) = (0..bytes.len().saturating_sub(1))
        .rev()
        .find(|&i| bytes[i] == b'\\' && bytes[i + 1] == b'\\')
    {
        let name = &input[i + 2..];
        let scope = &input[..i + 2];
        return Some((name, scope, "\\"));
    }

    // Priority 3b: \ followed by ASCII letter (backward compat for PHP namespaces)
    if let Some(i) = (0..bytes.len().saturating_sub(1))
        .rev()
        .find(|&i| bytes[i] == b'\\' && bytes[i + 1].is_ascii_alphabetic())
    {
        let name = &input[i + 1..];
        let scope = &input[..i + 1];
        return Some((name, scope, "\\"));
    }

    None
}

fn find_escaped_dot(input: &str) -> Option<usize> {
    let bytes = input.as_bytes();
    (0..bytes.len().saturating_sub(1))
        .rev()
        .find(|&i| bytes[i] == b'\\' && bytes[i + 1] == b'.')
}

#[cfg(test)]
mod tests {
    use super::*;

    // ===== Exact mode (unchanged) =====

    #[test]
    fn exact_mode_no_operators() {
        match MatchMode::from_user_input("my_func", false) {
            Ok(MatchMode::Exact { name, .. }) => assert_eq!(name, "my_func"),
            _ => panic!("expected Exact mode"),
        }
    }

    #[test]
    fn exact_mode_plain_name() {
        match MatchMode::from_user_input("ab", false) {
            Ok(MatchMode::Exact { name, .. }) => assert_eq!(name, "ab"),
            _ => panic!("expected Exact mode"),
        }
    }

    // ===== Fuzzy trigger expansion: all regex metacharacters =====

    #[test]
    fn fuzzy_triggered_by_plus() {
        let mode = MatchMode::from_user_input("get+", false).unwrap();
        assert!(matches!(mode, MatchMode::Fuzzy { .. }));
    }

    #[test]
    fn fuzzy_triggered_by_question() {
        let mode = MatchMode::from_user_input("get?", false).unwrap();
        assert!(matches!(mode, MatchMode::Fuzzy { .. }));
    }

    #[test]
    fn fuzzy_triggered_by_bracket() {
        let mode = MatchMode::from_user_input("get[set]", false).unwrap();
        assert!(matches!(mode, MatchMode::Fuzzy { .. }));
    }

    #[test]
    fn fuzzy_triggered_by_brace() {
        let mode = MatchMode::from_user_input("a{2,4}", false).unwrap();
        assert!(matches!(mode, MatchMode::Fuzzy { .. }));
    }

    #[test]
    fn fuzzy_triggered_by_caret() {
        let mode = MatchMode::from_user_input("a^b", false).unwrap();
        assert!(matches!(mode, MatchMode::Fuzzy { .. }));
    }

    #[test]
    fn fuzzy_triggered_by_dollar() {
        let mode = MatchMode::from_user_input("a$b", false).unwrap();
        assert!(matches!(mode, MatchMode::Fuzzy { .. }));
    }

    #[test]
    fn fuzzy_triggered_by_backslash_word() {
        // \w+ triggers Fuzzy (via \), then HIR analysis routes to All
        let mode = MatchMode::from_user_input("\\w+", false).unwrap();
        assert!(matches!(mode, MatchMode::All));
    }

    // ===== Standard . semantics: matches any character =====

    #[test]
    fn fuzzy_dot_matches_any_char_including_space() {
        let mode = MatchMode::from_user_input("My.*ss", false).unwrap();
        match &mode {
            MatchMode::Fuzzy { compiled, .. } => {
                assert!(compiled.is_match("MyProcess"));
                assert!(compiled.is_match("MyAccess"));
                assert!(compiled.is_match("My bar ss")); // space matched by .
                assert!(!compiled.is_match("MyHandler"));
            }
            _ => panic!("expected Fuzzy mode"),
        }
    }

    #[test]
    fn fuzzy_single_dot_matches_space() {
        let mode = MatchMode::from_user_input("a.b", false).unwrap();
        match &mode {
            MatchMode::Fuzzy { compiled, .. } => {
                assert!(compiled.is_match("aXb"));
                assert!(compiled.is_match("a b")); // space
                assert!(!compiled.is_match("ab")); // need exactly one char
            }
            _ => panic!("expected Fuzzy mode"),
        }
    }

    // ===== No broad pattern rejection =====

    #[test]
    fn accept_dot_star_as_all() {
        let mode = MatchMode::from_user_input(".*", false).unwrap();
        assert!(matches!(mode, MatchMode::All));
    }

    #[test]
    fn accept_dot_plus_as_all() {
        let mode = MatchMode::from_user_input(".+", false).unwrap();
        assert!(matches!(mode, MatchMode::All));
    }

    #[test]
    fn accept_a_dot_star_as_fuzzy() {
        let mode = MatchMode::from_user_input("a.*", false).unwrap();
        // a.* is not "all match" — it requires leading 'a'
        assert!(matches!(mode, MatchMode::Fuzzy { .. }));
    }

    // ===== HIR Impossible detection =====

    #[test]
    fn reject_pure_whitespace_as_impossible() {
        let result = MatchMode::from_user_input("\\s+", false);
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
        let result = MatchMode::from_user_input("\\W", false);
        assert!(result.is_err());
    }

    #[test]
    fn accept_alternation_with_valid_branch() {
        let mode = MatchMode::from_user_input("\\s|foo", false).unwrap();
        assert!(matches!(mode, MatchMode::Fuzzy { .. }));
        if let MatchMode::Fuzzy { compiled, .. } = &mode {
            assert!(compiled.is_match("foo"));
        }
    }

    // ===== HIR All-match detection =====

    #[test]
    fn all_mode_word_plus() {
        let mode = MatchMode::from_user_input("\\w+", false).unwrap();
        assert!(matches!(mode, MatchMode::All));
    }

    #[test]
    fn all_mode_word_star() {
        let mode = MatchMode::from_user_input("\\w*", false).unwrap();
        assert!(matches!(mode, MatchMode::All));
    }

    #[test]
    fn all_mode_grouped_dot_star() {
        let mode = MatchMode::from_user_input("(.*)", false).unwrap();
        assert!(matches!(mode, MatchMode::All));
    }

    #[test]
    fn all_mode_noncap_grouped_word_plus() {
        let mode = MatchMode::from_user_input("(?:\\w+)", false).unwrap();
        assert!(matches!(mode, MatchMode::All));
    }

    #[test]
    fn not_all_mode_literal() {
        let mode = MatchMode::from_user_input("foo", false).unwrap();
        assert!(!matches!(mode, MatchMode::All));
    }

    #[test]
    fn not_all_mode_fuzzy_with_literals() {
        let mode = MatchMode::from_user_input("foo.*bar", false).unwrap();
        assert!(!matches!(mode, MatchMode::All));
        assert!(matches!(mode, MatchMode::Fuzzy { .. }));
    }

    // ===== Fuzzy compiled regex (standard regex) =====

    #[test]
    fn fuzzy_mode_pipe() {
        let mode = MatchMode::from_user_input("process|handle", false).unwrap();
        match &mode {
            MatchMode::Fuzzy { compiled, .. } => {
                assert!(compiled.is_match("process"));
                assert!(compiled.is_match("handle"));
                assert!(!compiled.is_match("processing"));
            }
            _ => panic!("expected Fuzzy mode"),
        }
    }

    #[test]
    fn fuzzy_mixed_pipe_star() {
        let mode = MatchMode::from_user_input("process.*|handle.*", false).unwrap();
        match &mode {
            MatchMode::Fuzzy { compiled, .. } => {
                assert!(compiled.is_match("process_data"));
                assert!(compiled.is_match("handle_event"));
                assert!(!compiled.is_match("update_data"));
            }
            _ => panic!("expected Fuzzy mode"),
        }
    }

    #[test]
    fn fuzzy_group_pattern() {
        let mode = MatchMode::from_user_input("(process|handle)_data", false).unwrap();
        match &mode {
            MatchMode::Fuzzy { compiled, .. } => {
                assert!(compiled.is_match("process_data"));
                assert!(compiled.is_match("handle_data"));
                assert!(!compiled.is_match("process_event"));
            }
            _ => panic!("expected Fuzzy mode"),
        }
    }

    #[test]
    fn fuzzy_word_class_pattern() {
        let mode = MatchMode::from_user_input("test_\\w+", false).unwrap();
        match &mode {
            MatchMode::Fuzzy { compiled, .. } => {
                assert!(compiled.is_match("test_func"));
                assert!(compiled.is_match("test_case"));
                assert!(!compiled.is_match("test_")); // \w+ requires at least one
            }
            _ => panic!("expected Fuzzy mode"),
        }
    }

    #[test]
    fn fuzzy_backslash_dot_literal() {
        // get\.name goes through scope detection (Priority 2: \.)
        // It becomes: name = "name" (Exact), scope = "get\." → matches "get.name"
        let parsed = ParsedPattern::parse("get\\.name", CaseSensitivity::Sensitive).unwrap();
        let filter = parsed.scope_filter().expect("should detect scope");
        assert_eq!(filter.separator(), ".");
        assert!(filter.matches_scope("get.name"));
        match parsed.mode() {
            MatchMode::Exact { name, .. } => assert_eq!(name, "name"),
            _ => panic!("expected Exact mode"),
        }
    }

    // ===== Invalid regex =====

    #[test]
    fn reject_invalid_regex() {
        let result = MatchMode::from_user_input("(ab", false);
        assert!(result.is_err());
    }

    #[test]
    fn reject_invalid_regex_close_paren() {
        let result = MatchMode::from_user_input("ab)", false);
        assert!(result.is_err());
    }

    // ===== matches_ident =====

    #[test]
    fn exact_matches_ident() {
        let mode = MatchMode::Exact {
            name: "foo".to_string(),
            case_insensitive: false,
        };
        assert!(mode.matches_ident("foo"));
        assert!(!mode.matches_ident("foobar"));
    }

    #[test]
    fn fuzzy_prevents_substring() {
        let mode = MatchMode::from_user_input("My.*er", false).unwrap();
        match &mode {
            MatchMode::Fuzzy { compiled, .. } => {
                assert!(compiled.is_match("MyHandler"));
                assert!(!compiled.is_match("MyHandlerFactory"));
            }
            _ => panic!("expected Fuzzy mode"),
        }
    }

    // ===== MatchMode::All =====

    #[test]
    fn all_matches_any_ident() {
        let mode = MatchMode::All;
        assert!(mode.matches_ident("anything"));
        assert!(mode.matches_ident(""));
        assert!(mode.matches_ident("MyClass"));
    }

    // ===== Default case-sensitive behavior =====

    #[test]
    fn default_exact_lowercase_stays_sensitive() {
        let mode = MatchMode::from_user_input("foo", false).unwrap();
        assert!(mode.matches_ident("foo"));
        assert!(!mode.matches_ident("Foo"));
        assert!(!mode.matches_ident("FOO"));
    }

    #[test]
    fn default_exact_uppercase_stays_sensitive() {
        let mode = MatchMode::from_user_input("Foo", false).unwrap();
        assert!(mode.matches_ident("Foo"));
        assert!(!mode.matches_ident("foo"));
        assert!(!mode.matches_ident("FOO"));
    }

    #[test]
    fn default_fuzzy_lowercase_stays_sensitive() {
        let mode = MatchMode::from_user_input("foo.*bar", false).unwrap();
        assert!(mode.matches_ident("foobar"));
        assert!(!mode.matches_ident("FooBar"));
        assert!(!mode.matches_ident("FOOBAR"));
    }

    #[test]
    fn default_fuzzy_mixed_case_stays_sensitive() {
        let mode = MatchMode::from_user_input("Foo.*bar", false).unwrap();
        assert!(mode.matches_ident("FooXYZbar"));
        assert!(!mode.matches_ident("FooBar"));
        assert!(!mode.matches_ident("foobar"));
    }

    // ===== -i / --ignore-case =====

    #[test]
    fn ignore_case_exact_matches_any_case() {
        let mode = MatchMode::from_user_input("foo", true).unwrap();
        assert!(mode.matches_ident("foo"));
        assert!(mode.matches_ident("Foo"));
        assert!(mode.matches_ident("FOO"));
    }

    #[test]
    fn ignore_case_fuzzy_matches_any_case() {
        let mode = MatchMode::from_user_input("foo.*bar", true).unwrap();
        assert!(mode.matches_ident("foobar"));
        assert!(mode.matches_ident("FooBar"));
        assert!(mode.matches_ident("FOOBAR"));
    }

    // ===== -S / --smart-case =====

    #[test]
    fn smart_case_exact_lowercase_insensitive() {
        let mode = MatchMode::from_user_input("foo", is_smart_case("foo")).unwrap();
        assert!(mode.matches_ident("foo"));
        assert!(mode.matches_ident("Foo"));
        assert!(mode.matches_ident("FOO"));
    }

    #[test]
    fn smart_case_exact_uppercase_sensitive() {
        let mode = MatchMode::from_user_input("Foo", is_smart_case("Foo")).unwrap();
        assert!(mode.matches_ident("Foo"));
        assert!(!mode.matches_ident("foo"));
    }

    #[test]
    fn smart_case_fuzzy_lowercase_insensitive() {
        let mode = MatchMode::from_user_input("foo.*bar", is_smart_case("foo.*bar")).unwrap();
        assert!(mode.matches_ident("foobar"));
        assert!(mode.matches_ident("FooBar"));
    }

    #[test]
    fn smart_case_fuzzy_uppercase_sensitive() {
        let mode = MatchMode::from_user_input("Foo.*bar", is_smart_case("Foo.*bar")).unwrap();
        assert!(mode.matches_ident("FooXYZbar"));
        assert!(!mode.matches_ident("FooBar"));
        assert!(!mode.matches_ident("foobar"));
    }

    // ===== Ellipsis (list-all) =====

    #[test]
    fn parse_ellipsis_returns_all_mode() {
        let parsed = ParsedPattern::parse("...", CaseSensitivity::Sensitive).unwrap();
        assert!(matches!(parsed.mode(), MatchMode::All));
        assert!(parsed.scope_filter().is_none());
        assert_eq!(parsed.display_name(), "...");
    }

    #[test]
    fn parse_ellipsis_not_triggered_by_two_dots() {
        // ".." = regex matching any 2 chars — valid Fuzzy, not list-all
        let parsed = ParsedPattern::parse("..", CaseSensitivity::Sensitive).unwrap();
        assert!(matches!(parsed.mode(), MatchMode::Fuzzy { .. }));
    }

    #[test]
    fn parse_ellipsis_not_triggered_by_four_dots() {
        // "...." = regex matching any 4 chars — valid Fuzzy, not list-all
        let parsed = ParsedPattern::parse("....", CaseSensitivity::Sensitive).unwrap();
        assert!(matches!(parsed.mode(), MatchMode::Fuzzy { .. }));
    }

    // ===== ParsedPattern / ScopeFilter =====

    #[test]
    fn parse_no_scope_exact() {
        let parsed = ParsedPattern::parse("my_func", CaseSensitivity::Sensitive).unwrap();
        assert!(parsed.scope_filter().is_none());
        match parsed.mode() {
            MatchMode::Exact { name, .. } => assert_eq!(name, "my_func"),
            _ => panic!("expected Exact mode"),
        }
    }

    #[test]
    fn parse_no_scope_fuzzy() {
        let parsed = ParsedPattern::parse("my.*func", CaseSensitivity::Sensitive).unwrap();
        assert!(parsed.scope_filter().is_none());
        assert!(matches!(parsed.mode(), MatchMode::Fuzzy { .. }));
    }

    // :: separator
    #[test]
    fn parse_double_colon_exact_scope() {
        let parsed = ParsedPattern::parse("MyClass::myfunc", CaseSensitivity::Sensitive).unwrap();
        let filter = parsed.scope_filter().expect("should have scope filter");
        assert_eq!(filter.separator(), "::");
        match parsed.mode() {
            MatchMode::Exact { name, .. } => assert_eq!(name, "myfunc"),
            _ => panic!("expected Exact mode for name"),
        }
        assert!(filter.matches_scope("MyClass::myfunc"));
        assert!(!filter.matches_scope("A::MyClass::myfunc"));
    }

    #[test]
    fn parse_double_colon_multi_level() {
        let parsed = ParsedPattern::parse("A::B::C::myfunc", CaseSensitivity::Sensitive).unwrap();
        let filter = parsed.scope_filter().unwrap();
        assert!(filter.matches_scope("A::B::C::myfunc"));
        assert!(!filter.matches_scope("A::B::myfunc"));
    }

    #[test]
    fn parse_double_colon_wildcard_scope() {
        let parsed = ParsedPattern::parse(".*::myfunc", CaseSensitivity::Sensitive).unwrap();
        let filter = parsed.scope_filter().unwrap();
        assert!(filter.matches_scope("A::B::myfunc"));
        assert!(filter.matches_scope("A::myfunc"));
        assert!(!filter.matches_scope("myfunc"));
    }

    #[test]
    fn parse_double_colon_fuzzy_name() {
        let parsed = ParsedPattern::parse("MyClass::my.*func", CaseSensitivity::Sensitive).unwrap();
        assert!(parsed.scope_filter().is_some());
        assert!(matches!(parsed.mode(), MatchMode::Fuzzy { .. }));
    }

    // \. separator (escaped dot)
    #[test]
    fn parse_escaped_dot_exact_scope() {
        let parsed = ParsedPattern::parse("myclass\\.myfunc", CaseSensitivity::Sensitive).unwrap();
        let filter = parsed.scope_filter().unwrap();
        assert_eq!(filter.separator(), ".");
        assert!(filter.matches_scope("myclass.myfunc"));
        assert!(!filter.matches_scope("other.myfunc"));
    }

    #[test]
    fn parse_escaped_dot_fuzzy_scope() {
        let parsed = ParsedPattern::parse(".*class\\.myfunc", CaseSensitivity::Sensitive).unwrap();
        let filter = parsed.scope_filter().unwrap();
        assert!(filter.matches_scope("myclass.myfunc"));
        assert!(filter.matches_scope("some.other.class.myfunc"));
        assert!(!filter.matches_scope("myfunc"));
    }

    // \ separator: backward compat (single backslash + letter)
    #[test]
    fn parse_backslash_scope_backward_compat() {
        // "App\Models\User" (single backslash + letter) — old syntax still works
        let parsed = ParsedPattern::parse("App\\Models\\User", CaseSensitivity::Sensitive).unwrap();
        let filter = parsed.scope_filter().unwrap();
        assert_eq!(filter.separator(), "\\");
        match parsed.mode() {
            MatchMode::Exact { name, .. } => assert_eq!(name, "User"),
            _ => panic!("expected Exact mode"),
        }
        assert!(filter.matches_scope("App\\Models\\User"));
        assert!(!filter.matches_scope("App\\Services\\User"));
    }

    // \\ separator: new syntax (double backslash)
    #[test]
    fn parse_double_backslash_scope() {
        // "App\\Models\\User" typed as "App\\\\Models\\\\User" in Rust literal
        // = actual string "App\\Models\\User" with double backslashes
        let parsed =
            ParsedPattern::parse("App\\\\Models\\\\User", CaseSensitivity::Sensitive).unwrap();
        let filter = parsed.scope_filter().unwrap();
        assert_eq!(filter.separator(), "\\");
        match parsed.mode() {
            MatchMode::Exact { name, .. } => assert_eq!(name, "User"),
            _ => panic!("expected Exact mode"),
        }
        // Definition scope uses single backslash: "App\Models\User"
        assert!(filter.matches_scope("App\\Models\\User"));
        assert!(!filter.matches_scope("App\\Services\\User"));
    }

    // Error cases
    #[test]
    fn parse_empty_name_error() {
        let result = ParsedPattern::parse("MyClass::", CaseSensitivity::Sensitive);
        assert!(result.is_err());
    }

    #[test]
    fn parse_invalid_scope_regex_error() {
        let result = ParsedPattern::parse("(unclosed::myfunc", CaseSensitivity::Sensitive);
        assert!(result.is_err());
    }

    // Scope + case sensitivity
    #[test]
    fn scope_filter_default_case_sensitive() {
        let parsed = ParsedPattern::parse("myclass::myfunc", CaseSensitivity::Sensitive).unwrap();
        let filter = parsed.scope_filter().unwrap();
        assert!(filter.matches_scope("myclass::myfunc"));
        assert!(!filter.matches_scope("MyClass::myfunc"));
    }

    #[test]
    fn scope_filter_ignore_case_insensitive() {
        let parsed = ParsedPattern::parse("myclass::myfunc", CaseSensitivity::Insensitive).unwrap();
        let filter = parsed.scope_filter().unwrap();
        assert!(filter.matches_scope("myclass::myfunc"));
        assert!(filter.matches_scope("MyClass::myfunc"));
        assert!(filter.matches_scope("MYCLASS::myfunc"));
    }

    #[test]
    fn scope_filter_smart_case_lowercase_insensitive() {
        let parsed = ParsedPattern::parse("myclass::myfunc", CaseSensitivity::SmartCase).unwrap();
        let filter = parsed.scope_filter().unwrap();
        assert!(filter.matches_scope("myclass::myfunc"));
        assert!(filter.matches_scope("MyClass::myfunc"));
    }

    #[test]
    fn scope_filter_smart_case_uppercase_sensitive() {
        let parsed = ParsedPattern::parse("MyClass::myfunc", CaseSensitivity::SmartCase).unwrap();
        let filter = parsed.scope_filter().unwrap();
        assert!(filter.matches_scope("MyClass::myfunc"));
        assert!(!filter.matches_scope("myclass::myfunc"));
    }

    #[test]
    fn parse_display_name_includes_full_input() {
        let parsed = ParsedPattern::parse("MyClass::myfunc", CaseSensitivity::Sensitive).unwrap();
        assert_eq!(parsed.display_name(), "MyClass::myfunc");
    }

    #[test]
    fn parse_display_name_no_scope() {
        let parsed = ParsedPattern::parse("myfunc", CaseSensitivity::Sensitive).unwrap();
        assert_eq!(parsed.display_name(), "myfunc");
    }

    // ===== CaseSensitivity integration =====

    #[test]
    fn parsed_pattern_sensitive_exact() {
        let parsed = ParsedPattern::parse("foo", CaseSensitivity::Sensitive).unwrap();
        assert!(parsed.mode().matches_ident("foo"));
        assert!(!parsed.mode().matches_ident("Foo"));
    }

    #[test]
    fn parsed_pattern_insensitive_exact() {
        let parsed = ParsedPattern::parse("foo", CaseSensitivity::Insensitive).unwrap();
        assert!(parsed.mode().matches_ident("foo"));
        assert!(parsed.mode().matches_ident("Foo"));
    }

    #[test]
    fn parsed_pattern_smart_case_lowercase() {
        let parsed = ParsedPattern::parse("foo", CaseSensitivity::SmartCase).unwrap();
        assert!(parsed.mode().matches_ident("foo"));
        assert!(parsed.mode().matches_ident("Foo"));
    }

    #[test]
    fn parsed_pattern_smart_case_uppercase() {
        let parsed = ParsedPattern::parse("Foo", CaseSensitivity::SmartCase).unwrap();
        assert!(parsed.mode().matches_ident("Foo"));
        assert!(!parsed.mode().matches_ident("foo"));
    }
}
