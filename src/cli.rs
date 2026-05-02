use crate::model::DefKind;
use clap::Parser;

/// Short flags that take a value (all others are boolean).
/// MUST stay in sync with Cli struct fields that have #[arg(short, long)] and take a value.
const VALUE_SHORT_FLAGS: &[char] = &['k', 'g', 'd', 'e'];
/// Long flags that take a value (all others are boolean).
/// MUST stay in sync with Cli struct fields that have #[arg(short, long)] and take a value.
const VALUE_LONG_FLAGS: &[&str] = &["kind", "glob", "max-depth", "regexp"];
/// Short flags that are boolean (no value).
/// MUST stay in sync with Cli struct fields that have #[arg(short)] and no value.
const BOOLEAN_SHORT_FLAGS: &[char] = &['i', 'S', 'l', 'c', 'w', 'H', 'I', 'M'];
/// Long flags that are boolean (no value).
/// MUST stay in sync with Cli struct fields that have #[arg(long)] and no value.
const BOOLEAN_LONG_FLAGS: &[&str] = &[
    "ignore-case",
    "smart-case",
    "no-signature",
    "files-with-matches",
    "count",
    "hidden",
    "no-ignore",
    "json",
    "no-messages",
    "word-regexp",
    "with-filename",
    "no-filename",
    "version",
    "help",
];

/// Check if `arg` looks like a known peek option (value-taking or boolean).
fn is_known_option(arg: &str) -> bool {
    if let Some(rest) = arg.strip_prefix("--") {
        if rest.is_empty() {
            return true; // "--" stop marker
        }
        let flag_name = rest.split_once('=').map(|(n, _)| n).unwrap_or(rest);
        VALUE_LONG_FLAGS.contains(&flag_name) || BOOLEAN_LONG_FLAGS.contains(&flag_name)
    } else if let Some(rest) = arg.strip_prefix("-") {
        if rest.is_empty() {
            return false; // bare "-" is stdin, not an option
        }
        let first = rest.chars().next().unwrap();
        VALUE_SHORT_FLAGS.contains(&first) || BOOLEAN_SHORT_FLAGS.contains(&first)
    } else {
        false
    }
}

/// Reorder CLI arguments so options appear before positional arguments.
///
/// This allows users to write `peek my_func src/ -k function` which clap's
/// `trailing_var_arg` would otherwise misparse — once `files` starts consuming,
/// all subsequent args (including `-k function`) are treated as file paths.
///
/// The function skips the program name (first arg), moves option groups
/// (`-x value`, `--flag=value`, `--flag value`, boolean flags) before
/// positional args, and respects `--` as a stop marker.
///
/// Values starting with `-` that are not known peek options are accepted and
/// converted to `--flag=value` syntax so clap parses them correctly.
pub fn reorder_cli_args(args: &[String]) -> Vec<String> {
    if args.len() <= 1 {
        return args.to_vec();
    }

    let program = &args[0];
    let mut opts = Vec::new();
    let mut positionals = Vec::new();
    let mut i = 1;

    while i < args.len() {
        let arg = &args[i];

        // Stop at -- separator
        if arg == "--" {
            positionals.push(arg.clone());
            positionals.extend(args[i + 1..].iter().cloned());
            break;
        }

        // Long option: --flag or --flag=value
        if let Some(rest) = arg.strip_prefix("--") {
            if !rest.contains('=')
                && VALUE_LONG_FLAGS.contains(&rest)
                && i + 1 < args.len()
                && !is_known_option(&args[i + 1])
            {
                let value = args[i + 1].clone();
                if value.starts_with('-') {
                    opts.push(format!("{}={}", arg, value));
                } else {
                    opts.push(arg.clone());
                    opts.push(value);
                }
                i += 1;
            } else if !rest.contains('=') && VALUE_LONG_FLAGS.contains(&rest) && i + 1 >= args.len()
            {
                positionals.push(arg.clone());
            } else {
                opts.push(arg.clone());
            }
        }
        // Short option: -x, -xvalue, or -xyz
        else if let Some(rest) = arg.strip_prefix("-") {
            if rest.is_empty() {
                positionals.push(arg.clone());
            } else if rest.len() == 1
                && VALUE_SHORT_FLAGS.contains(&rest.chars().next().unwrap())
                && i + 1 < args.len()
                && !is_known_option(&args[i + 1])
            {
                let value = args[i + 1].clone();
                if value.starts_with('-') {
                    opts.push(format!("{}={}", arg, value));
                } else {
                    opts.push(arg.clone());
                    opts.push(value);
                }
                i += 1;
            } else if rest.len() == 1
                && VALUE_SHORT_FLAGS.contains(&rest.chars().next().unwrap())
                && i + 1 >= args.len()
            {
                positionals.push(arg.clone());
            } else if rest.len() > 1 {
                let chars: Vec<char> = rest.chars().collect();
                let last = *chars.last().unwrap();
                if VALUE_SHORT_FLAGS.contains(&last) {
                    for &c in &chars[..chars.len() - 1] {
                        opts.push(format!("-{}", c));
                    }
                    if i + 1 < args.len() && !is_known_option(&args[i + 1]) {
                        let value = args[i + 1].clone();
                        if value.starts_with('-') {
                            opts.push(format!("-{}={}", last, value));
                        } else {
                            opts.push(format!("-{}", last));
                            opts.push(value);
                        }
                        i += 1;
                    } else if i + 1 >= args.len() {
                        positionals.push(format!("-{}", last));
                    } else {
                        opts.push(format!("-{}", last));
                    }
                } else {
                    opts.push(arg.clone());
                }
            } else {
                opts.push(arg.clone());
            }
        }
        // Positional
        else {
            positionals.push(arg.clone());
        }

        i += 1;
    }

    std::iter::once(program.clone())
        .chain(opts)
        .chain(positionals)
        .collect()
}

#[derive(Parser)]
#[command(
    name = "peek",
    bin_name = "peek",
    about = "Search code definitions by name",
    version
)]
pub struct Cli {
    /// Definition types to search for (comma-separated: function,class,struct)
    #[arg(short = 'k', long = "kind", value_delimiter = ',')]
    kind: Option<Vec<String>>,

    /// Ignore case distinctions in the pattern
    #[arg(short = 'i', long = "ignore-case")]
    ignore_case: bool,

    /// Smart case: ignore case unless the pattern contains uppercase letters
    #[arg(short = 'S', long = "smart-case", conflicts_with = "ignore_case")]
    smart_case: bool,

    /// Suppress signature output
    #[arg(long = "no-signature")]
    no_signature: bool,

    /// Only show file paths with matches, not the matched definitions
    #[arg(short = 'l', long = "files-with-matches", conflicts_with = "count")]
    files_with_matches: bool,

    /// Show count of matching definitions per file instead of the definitions
    #[arg(short = 'c', long = "count", conflicts_with = "files_with_matches")]
    count: bool,

    /// Search hidden files and directories
    #[arg(long = "hidden")]
    hidden: bool,

    /// Do not respect .gitignore and .ignore rules
    #[arg(long = "no-ignore")]
    no_ignore: bool,

    /// Maximum directory traversal depth
    #[arg(short = 'd', long = "max-depth")]
    max_depth: Option<usize>,

    /// Show results in JSON format (envelope format compatible with ripgrep --json)
    #[arg(long = "json")]
    json: bool,

    /// Suppress non-fatal error messages (traversal, read, and parse errors)
    #[arg(short = 'M', long = "no-messages")]
    no_messages: bool,

    /// Only match whole words (surround pattern with word boundaries)
    #[arg(short = 'w', long = "word-regexp")]
    word: bool,

    /// Always show file path with results (overrides --no-filename)
    #[arg(short = 'H', long = "with-filename", conflicts_with = "no_filename")]
    with_filename: bool,

    /// Never show file path with results (overrides --with-filename)
    #[arg(short = 'I', long = "no-filename", conflicts_with = "with_filename")]
    no_filename: bool,

    /// Glob patterns to filter files (e.g., -g '*.rs', -g '!*.test.rs')
    /// Later globs override earlier globs; '!' prefix negates.
    #[arg(short = 'g', long = "glob")]
    glob: Vec<String>,

    /// Search patterns (can be specified multiple times)
    #[arg(short = 'e', long = "regexp")]
    regexp: Vec<String>,

    /// Definition name to search for (supports exact and fuzzy matching)
    /// Optional when -e/--regexp is used.
    pattern: Option<String>,

    /// Files or directories to search in (default: current directory)
    #[arg(trailing_var_arg = true)]
    files: Vec<String>,
}

impl Cli {
    #[cfg(test)]
    pub fn pattern(&self) -> Option<&str> {
        self.pattern.as_deref()
    }

    #[cfg(test)]
    pub fn regexp(&self) -> &Vec<String> {
        &self.regexp
    }

    pub fn collect_patterns(&self) -> Vec<String> {
        if !self.regexp.is_empty() {
            return self.regexp.clone();
        }
        if let Some(p) = &self.pattern {
            vec![p.clone()]
        } else {
            vec![]
        }
    }

    pub fn files(&self) -> Vec<String> {
        if !self.regexp.is_empty() {
            let mut files = Vec::new();
            if let Some(p) = &self.pattern {
                files.push(p.clone());
            }
            files.extend(self.files.iter().cloned());
            files
        } else {
            self.files.clone()
        }
    }

    pub fn kinds(&self) -> Vec<DefKind> {
        match &self.kind {
            Some(tags) => tags.iter().filter_map(|t| DefKind::from_tag(t)).collect(),
            None => DefKind::all().to_vec(),
        }
    }

    pub fn no_signature(&self) -> bool {
        self.no_signature
    }

    pub fn ignore_case(&self) -> bool {
        self.ignore_case
    }

    pub fn smart_case(&self) -> bool {
        self.smart_case
    }

    pub fn globs(&self) -> &Vec<String> {
        &self.glob
    }

    pub fn kind_tags(&self) -> Option<&Vec<String>> {
        self.kind.as_ref()
    }

    pub fn files_with_matches(&self) -> bool {
        self.files_with_matches
    }

    pub fn count(&self) -> bool {
        self.count
    }

    pub fn hidden(&self) -> bool {
        self.hidden
    }

    pub fn no_ignore(&self) -> bool {
        self.no_ignore
    }

    pub fn no_messages(&self) -> bool {
        self.no_messages
    }

    pub fn json(&self) -> bool {
        self.json
    }

    pub fn max_depth(&self) -> Option<usize> {
        self.max_depth
    }

    pub fn with_filename(&self) -> bool {
        self.with_filename
    }

    pub fn no_filename(&self) -> bool {
        self.no_filename
    }

    pub fn word(&self) -> bool {
        self.word
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_pattern_only() {
        let cli = Cli::try_parse_from(["peek", "my_func"]).unwrap();
        assert_eq!(cli.pattern(), Some("my_func"));
        assert!(cli.files().is_empty());
        assert!(cli.kinds() == DefKind::all().to_vec());
        assert!(!cli.no_signature());
    }

    #[test]
    fn parse_pattern_with_single_file() {
        let cli = Cli::try_parse_from(["peek", "my_func", "src/"]).unwrap();
        assert_eq!(cli.pattern(), Some("my_func"));
        assert_eq!(cli.files(), &["src/".to_string()]);
    }

    #[test]
    fn parse_pattern_with_multiple_files() {
        let cli = Cli::try_parse_from(["peek", "my_func", "main.cpp", "math.cpp"]).unwrap();
        assert_eq!(cli.pattern(), Some("my_func"));
        assert_eq!(
            cli.files(),
            &["main.cpp".to_string(), "math.cpp".to_string()]
        );
    }

    #[test]
    fn parse_pattern_with_glob() {
        let cli = Cli::try_parse_from(["peek", "Config", "*.rs"]).unwrap();
        assert_eq!(cli.pattern(), Some("Config"));
        assert_eq!(cli.files(), &["*.rs".to_string()]);
    }

    #[test]
    fn parse_kind_short() {
        let cli = Cli::try_parse_from(["peek", "-k", "class", "Pipeline"]).unwrap();
        assert_eq!(cli.pattern(), Some("Pipeline"));
        assert!(cli.files().is_empty());
        assert_eq!(cli.kinds(), vec![DefKind::Class]);
    }

    #[test]
    fn parse_kind_long() {
        let cli = Cli::try_parse_from(["peek", "--kind", "class", "Pipeline"]).unwrap();
        assert_eq!(cli.pattern(), Some("Pipeline"));
        assert_eq!(cli.kinds(), vec![DefKind::Class]);
    }

    #[test]
    fn parse_kind_multiple() {
        let cli = Cli::try_parse_from(["peek", "-k", "struct,class", "Config"]).unwrap();
        assert_eq!(cli.kinds(), vec![DefKind::Struct, DefKind::Class]);
    }

    #[test]
    fn parse_all_flags_with_files() {
        let cli = Cli::try_parse_from(["peek", "-k", "struct", "--no-signature", "Config", "src/"])
            .unwrap();
        assert_eq!(cli.pattern(), Some("Config"));
        assert_eq!(cli.files(), &["src/".to_string()]);
        assert_eq!(cli.kinds(), vec![DefKind::Struct]);
        assert!(cli.no_signature());
    }

    #[test]
    fn parse_no_signature() {
        let cli = Cli::try_parse_from(["peek", "--no-signature", "foo"]).unwrap();
        assert!(cli.no_signature());
    }

    #[test]
    fn all_cli_defaults_correct() {
        let cli = Cli::try_parse_from(["peek", "foo"]).unwrap();
        assert!(!cli.no_signature());
        assert!(cli.kind_tags().is_none());
        assert!(cli.globs().is_empty());
        assert!(!cli.ignore_case());
        assert!(!cli.smart_case());
        assert_eq!(cli.max_depth(), None);
        assert!(!cli.files_with_matches());
        assert!(!cli.count());
        assert!(!cli.hidden());
        assert!(!cli.no_ignore());
        assert!(!cli.json());
        assert!(!cli.no_messages());
        assert!(!cli.with_filename());
        assert!(!cli.no_filename());
        assert!(!cli.word());
        assert!(cli.regexp().is_empty());
    }

    #[test]
    fn accept_no_pattern_when_regexp_provided() {
        let cli = Cli::try_parse_from(["peek"]).unwrap();
        assert!(cli.pattern().is_none());
        assert!(cli.regexp().is_empty());
    }

    #[test]
    fn reject_old_in_flag() {
        assert!(Cli::try_parse_from(["peek", "foo", "--in", "src/"]).is_err());
    }

    #[test]
    fn reject_old_only_flag() {
        assert!(Cli::try_parse_from(["peek", "foo", "--only", "class"]).is_err());
    }

    #[test]
    fn kinds_default_returns_all() {
        let cli = Cli::try_parse_from(["peek", "test"]).unwrap();
        assert_eq!(cli.kinds(), DefKind::all().to_vec());
    }

    #[test]
    fn kinds_single_tag() {
        let cli = Cli::try_parse_from(["peek", "-k", "function", "test"]).unwrap();
        assert_eq!(cli.kinds(), vec![DefKind::Function]);
    }

    #[test]
    fn kinds_multiple_tags() {
        let cli = Cli::try_parse_from(["peek", "-k", "function,class", "test"]).unwrap();
        assert_eq!(cli.kinds(), vec![DefKind::Function, DefKind::Class]);
    }

    #[test]
    fn kind_tags_returns_raw() {
        let cli = Cli::try_parse_from(["peek", "-k", "function", "test"]).unwrap();
        assert_eq!(cli.kind_tags(), Some(&vec!["function".to_string()]));
    }

    #[test]
    fn version_flag() {
        assert!(Cli::try_parse_from(["peek", "--version"]).is_err());
    }

    #[test]
    fn help_flag() {
        assert!(Cli::try_parse_from(["peek", "--help"]).is_err());
    }

    #[test]
    fn options_after_pattern_still_recognized() {
        let cli = Cli::try_parse_from(["peek", "my_func", "-k", "function", "src/"]).unwrap();
        assert_eq!(cli.pattern(), Some("my_func"));
        assert_eq!(cli.files(), &["src/".to_string()]);
        assert_eq!(cli.kinds(), vec![DefKind::Function]);
    }

    #[test]
    fn glob_flag_single() {
        let cli = Cli::try_parse_from(["peek", "-g", "*.rs", "Config"]).unwrap();
        assert_eq!(cli.pattern(), Some("Config"));
        assert_eq!(cli.globs(), &["*.rs".to_string()]);
    }

    #[test]
    fn glob_flag_long() {
        let cli = Cli::try_parse_from(["peek", "--glob", "*.rs", "Config"]).unwrap();
        assert_eq!(cli.pattern(), Some("Config"));
        assert_eq!(cli.globs(), &["*.rs".to_string()]);
    }

    #[test]
    fn glob_flag_multiple() {
        let cli =
            Cli::try_parse_from(["peek", "-g", "*.rs", "-g", "!*.test.rs", "Config"]).unwrap();
        assert_eq!(cli.pattern(), Some("Config"));
        assert_eq!(cli.globs(), &["*.rs".to_string(), "!*.test.rs".to_string()]);
    }

    #[test]
    fn glob_flag_with_negation() {
        let cli = Cli::try_parse_from(["peek", "-g", "!*.generated.rs", "Config"]).unwrap();
        assert_eq!(cli.globs(), &["!*.generated.rs".to_string()]);
    }

    #[test]
    fn glob_flag_with_path_and_kind() {
        let cli =
            Cli::try_parse_from(["peek", "-k", "class", "-g", "*.rs", "Config", "src/"]).unwrap();
        assert_eq!(cli.pattern(), Some("Config"));
        assert_eq!(cli.kinds(), vec![DefKind::Class]);
        assert_eq!(cli.globs(), &["*.rs".to_string()]);
        assert_eq!(cli.files(), &["src/".to_string()]);
    }

    // --- Case sensitivity flags ---

    #[test]
    fn parse_ignore_case_short() {
        let cli = Cli::try_parse_from(["peek", "-i", "foo"]).unwrap();
        assert!(cli.ignore_case());
        assert!(!cli.smart_case());
    }

    #[test]
    fn parse_ignore_case_long() {
        let cli = Cli::try_parse_from(["peek", "--ignore-case", "foo"]).unwrap();
        assert!(cli.ignore_case());
    }

    #[test]
    fn parse_smart_case_short() {
        let cli = Cli::try_parse_from(["peek", "-S", "foo"]).unwrap();
        assert!(!cli.ignore_case());
        assert!(cli.smart_case());
    }

    #[test]
    fn parse_smart_case_long() {
        let cli = Cli::try_parse_from(["peek", "--smart-case", "foo"]).unwrap();
        assert!(cli.smart_case());
    }

    #[test]
    fn reject_both_ignore_and_smart_case() {
        assert!(Cli::try_parse_from(["peek", "-i", "-S", "foo"]).is_err());
    }

    // --- New parameter tests ---

    #[test]
    fn files_with_matches_short() {
        let cli = Cli::try_parse_from(["peek", "-l", "foo"]).unwrap();
        assert!(cli.files_with_matches());
        assert!(!cli.count());
    }

    #[test]
    fn files_with_matches_long() {
        let cli = Cli::try_parse_from(["peek", "--files-with-matches", "foo"]).unwrap();
        assert!(cli.files_with_matches());
    }

    #[test]
    fn count_short() {
        let cli = Cli::try_parse_from(["peek", "-c", "foo"]).unwrap();
        assert!(cli.count());
        assert!(!cli.files_with_matches());
    }

    #[test]
    fn count_long() {
        let cli = Cli::try_parse_from(["peek", "--count", "foo"]).unwrap();
        assert!(cli.count());
    }

    #[test]
    fn reject_files_with_matches_and_count() {
        assert!(Cli::try_parse_from(["peek", "-l", "-c", "foo"]).is_err());
    }

    #[test]
    fn hidden_flag() {
        let cli = Cli::try_parse_from(["peek", "--hidden", "foo"]).unwrap();
        assert!(cli.hidden());
    }

    #[test]
    fn no_ignore_flag() {
        let cli = Cli::try_parse_from(["peek", "--no-ignore", "foo"]).unwrap();
        assert!(cli.no_ignore());
    }

    #[test]
    fn max_depth_short() {
        let cli = Cli::try_parse_from(["peek", "-d", "3", "foo"]).unwrap();
        assert_eq!(cli.max_depth(), Some(3));
    }

    #[test]
    fn max_depth_long() {
        let cli = Cli::try_parse_from(["peek", "--max-depth", "5", "foo"]).unwrap();
        assert_eq!(cli.max_depth(), Some(5));
    }

    #[test]
    fn combined_flags_with_existing() {
        let cli = Cli::try_parse_from([
            "peek", "-l", "-k", "class", "-g", "*.rs", "--hidden", "Config",
        ])
        .unwrap();
        assert!(cli.files_with_matches());
        assert_eq!(cli.kinds(), vec![DefKind::Class]);
        assert_eq!(cli.globs(), &["*.rs".to_string()]);
        assert!(cli.hidden());
        assert_eq!(cli.pattern(), Some("Config"));
    }

    // --- --json flag ---

    #[test]
    fn json_flag() {
        let cli = Cli::try_parse_from(["peek", "--json", "foo"]).unwrap();
        assert!(cli.json());
    }

    #[test]
    fn json_with_other_flags() {
        let cli = Cli::try_parse_from(["peek", "--json", "-l", "-k", "class", "Config"]).unwrap();
        assert!(cli.json());
        assert!(cli.files_with_matches());
        assert_eq!(cli.kinds(), vec![DefKind::Class]);
    }

    // --- --no-messages flag ---

    #[test]
    fn no_messages_short() {
        let cli = Cli::try_parse_from(["peek", "-M", "foo"]).unwrap();
        assert!(cli.no_messages());
    }

    #[test]
    fn no_messages_long() {
        let cli = Cli::try_parse_from(["peek", "--no-messages", "foo"]).unwrap();
        assert!(cli.no_messages());
    }

    // --- --with-filename / --no-filename flags ---

    #[test]
    fn with_filename_short() {
        let cli = Cli::try_parse_from(["peek", "-H", "foo"]).unwrap();
        assert!(cli.with_filename());
        assert!(!cli.no_filename());
    }

    #[test]
    fn with_filename_long() {
        let cli = Cli::try_parse_from(["peek", "--with-filename", "foo"]).unwrap();
        assert!(cli.with_filename());
    }

    #[test]
    fn no_filename_short() {
        let cli = Cli::try_parse_from(["peek", "-I", "foo"]).unwrap();
        assert!(!cli.with_filename());
        assert!(cli.no_filename());
    }

    #[test]
    fn no_filename_long() {
        let cli = Cli::try_parse_from(["peek", "--no-filename", "foo"]).unwrap();
        assert!(cli.no_filename());
    }

    #[test]
    fn reject_both_with_and_no_filename() {
        assert!(Cli::try_parse_from(["peek", "-H", "-I", "foo"]).is_err());
    }

    // --- --word-regexp flag ---

    #[test]
    fn word_regexp_short() {
        let cli = Cli::try_parse_from(["peek", "-w", "foo"]).unwrap();
        assert!(cli.word());
    }

    #[test]
    fn word_regexp_long() {
        let cli = Cli::try_parse_from(["peek", "--word-regexp", "foo"]).unwrap();
        assert!(cli.word());
    }

    // --- -e/--regexp flag ---

    #[test]
    fn regexp_flag_short() {
        let cli = Cli::try_parse_from(["peek", "-e", "foo"]).unwrap();
        assert_eq!(cli.regexp(), &["foo".to_string()]);
        assert!(cli.pattern().is_none());
    }

    #[test]
    fn regexp_flag_long() {
        let cli = Cli::try_parse_from(["peek", "--regexp", "foo"]).unwrap();
        assert_eq!(cli.regexp(), &["foo".to_string()]);
    }

    #[test]
    fn regexp_flag_multiple() {
        let cli = Cli::try_parse_from(["peek", "-e", "foo", "-e", "bar"]).unwrap();
        assert_eq!(cli.regexp(), &["foo".to_string(), "bar".to_string()]);
    }

    #[test]
    fn regexp_with_positional_pattern() {
        let cli = Cli::try_parse_from(["peek", "baz", "-e", "foo"]).unwrap();
        assert_eq!(cli.pattern(), Some("baz"));
        assert_eq!(cli.regexp(), &["foo".to_string()]);
    }

    #[test]
    fn collect_patterns_positional_only() {
        let cli = Cli::try_parse_from(["peek", "foo"]).unwrap();
        assert_eq!(cli.collect_patterns(), vec!["foo".to_string()]);
    }

    #[test]
    fn collect_patterns_regexp_only() {
        let cli = Cli::try_parse_from(["peek", "-e", "foo", "-e", "bar"]).unwrap();
        assert_eq!(
            cli.collect_patterns(),
            vec!["foo".to_string(), "bar".to_string()]
        );
    }

    #[test]
    fn collect_patterns_mixed() {
        // When -e is present, positional pattern is excluded (treated as path)
        let cli = Cli::try_parse_from(["peek", "baz", "-e", "foo", "-e", "bar"]).unwrap();
        assert_eq!(
            cli.collect_patterns(),
            vec!["foo".to_string(), "bar".to_string()]
        );
    }

    #[test]
    fn collect_patterns_empty_when_none() {
        let cli = Cli::try_parse_from(["peek"]).unwrap();
        assert!(cli.collect_patterns().is_empty());
    }

    // --- -e turns positional into path (ripgrep alignment) ---

    #[test]
    fn regexp_turns_positional_into_path() {
        let cli = Cli::try_parse_from(["peek", "src/", "-e", "foo"]).unwrap();
        assert_eq!(cli.collect_patterns(), vec!["foo".to_string()]);
        assert_eq!(cli.files(), &["src/".to_string()]);
    }

    #[test]
    fn regexp_positional_and_trailing_files_merged() {
        let args = args_from(&["peek", "src/", "lib/", "-e", "foo"]);
        let cli = Cli::try_parse_from(reorder_cli_args(&args)).unwrap();
        assert_eq!(cli.collect_patterns(), vec!["foo".to_string()]);
        assert_eq!(cli.files(), &["src/".to_string(), "lib/".to_string()]);
    }

    #[test]
    fn regexp_without_positional_empty_files() {
        let cli = Cli::try_parse_from(["peek", "-e", "foo"]).unwrap();
        assert_eq!(cli.collect_patterns(), vec!["foo".to_string()]);
        assert!(cli.files().is_empty());
    }

    #[test]
    fn no_regexp_positional_stays_as_pattern() {
        let cli = Cli::try_parse_from(["peek", "foo", "src/"]).unwrap();
        assert_eq!(cli.collect_patterns(), vec!["foo".to_string()]);
        assert_eq!(cli.files(), &["src/".to_string()]);
    }

    // --- reorder_cli_args ---

    fn args_from(raw: &[&str]) -> Vec<String> {
        raw.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn reorder_no_options_unchanged() {
        let args = args_from(&["peek", "my_func", "src/"]);
        assert_eq!(
            reorder_cli_args(&args),
            args_from(&["peek", "my_func", "src/"])
        );
    }

    #[test]
    fn reorder_already_correct_order() {
        let args = args_from(&["peek", "-k", "function", "my_func", "src/"]);
        assert_eq!(
            reorder_cli_args(&args),
            args_from(&["peek", "-k", "function", "my_func", "src/"])
        );
    }

    #[test]
    fn reorder_kind_after_files() {
        let args = args_from(&["peek", "my_func", "src/", "-k", "function"]);
        assert_eq!(
            reorder_cli_args(&args),
            args_from(&["peek", "-k", "function", "my_func", "src/"])
        );
    }

    #[test]
    fn reorder_boolean_flag_after_files() {
        let args = args_from(&["peek", "my_func", "src/", "--hidden"]);
        assert_eq!(
            reorder_cli_args(&args),
            args_from(&["peek", "--hidden", "my_func", "src/"])
        );
    }

    #[test]
    fn reorder_multiple_options_after_files() {
        let args = args_from(&["peek", "my_func", "src/", "-k", "function", "--hidden"]);
        assert_eq!(
            reorder_cli_args(&args),
            args_from(&["peek", "-k", "function", "--hidden", "my_func", "src/"])
        );
    }

    #[test]
    fn reorder_glob_multi_value() {
        let args = args_from(&["peek", "my_func", "src/", "-g", "*.rs", "-g", "!*.test.rs"]);
        assert_eq!(
            reorder_cli_args(&args),
            args_from(&["peek", "-g", "*.rs", "-g", "!*.test.rs", "my_func", "src/"])
        );
    }

    #[test]
    fn reorder_preserves_positional_order() {
        let args = args_from(&["peek", "my_func", "src/", "lib/", "-k", "function"]);
        assert_eq!(
            reorder_cli_args(&args),
            args_from(&["peek", "-k", "function", "my_func", "src/", "lib/"])
        );
    }

    #[test]
    fn reorder_double_dash_stops() {
        let args = args_from(&["peek", "my_func", "--", "src/", "-k", "function"]);
        assert_eq!(
            reorder_cli_args(&args),
            args_from(&["peek", "my_func", "--", "src/", "-k", "function"])
        );
    }

    #[test]
    fn reorder_long_equals_value() {
        let args = args_from(&["peek", "my_func", "src/", "--kind=function"]);
        assert_eq!(
            reorder_cli_args(&args),
            args_from(&["peek", "--kind=function", "my_func", "src/"])
        );
    }

    #[test]
    fn reorder_long_space_value() {
        let args = args_from(&["peek", "my_func", "src/", "--kind", "function"]);
        assert_eq!(
            reorder_cli_args(&args),
            args_from(&["peek", "--kind", "function", "my_func", "src/"])
        );
    }

    #[test]
    fn reorder_max_depth() {
        let args = args_from(&["peek", "my_func", "src/", "-d", "3"]);
        assert_eq!(
            reorder_cli_args(&args),
            args_from(&["peek", "-d", "3", "my_func", "src/"])
        );
    }

    #[test]
    fn reorder_combined_short_with_value() {
        let args = args_from(&["peek", "my_func", "src/", "-kfunction"]);
        assert_eq!(
            reorder_cli_args(&args),
            args_from(&["peek", "-kfunction", "my_func", "src/"])
        );
    }

    #[test]
    fn reorder_cluster_boolean_plus_value_flag() {
        // -ik function means -i (boolean) + -k function
        let args = args_from(&["peek", "my_func", "src/", "-ik", "function"]);
        assert_eq!(
            reorder_cli_args(&args),
            args_from(&["peek", "-i", "-k", "function", "my_func", "src/"])
        );
    }

    #[test]
    fn reorder_cluster_boolean_plus_value_flag_no_value() {
        // -ik at end with no value → split: -i to opts, -k to positionals
        let args = args_from(&["peek", "my_func", "-ik"]);
        assert_eq!(
            reorder_cli_args(&args),
            args_from(&["peek", "-i", "my_func", "-k"])
        );
    }

    #[test]
    fn reorder_cluster_boolean_plus_value_flag_dash_value() {
        // -ik -S where -S is a known boolean flag → split but don't grab
        let args = args_from(&["peek", "my_func", "-ik", "-S"]);
        assert_eq!(
            reorder_cli_args(&args),
            args_from(&["peek", "-i", "-k", "-S", "my_func"])
        );
    }

    #[test]
    fn reorder_regexp_after_files() {
        let args = args_from(&["peek", "my_func", "src/", "-e", "pattern"]);
        assert_eq!(
            reorder_cli_args(&args),
            args_from(&["peek", "-e", "pattern", "my_func", "src/"])
        );
    }

    #[test]
    fn reorder_multiple_regexp() {
        let args = args_from(&["peek", "src/", "-e", "foo", "-e", "bar"]);
        assert_eq!(
            reorder_cli_args(&args),
            args_from(&["peek", "-e", "foo", "-e", "bar", "src/"])
        );
    }

    #[test]
    fn reorder_single_program_arg() {
        let args = args_from(&["peek"]);
        assert_eq!(reorder_cli_args(&args), args_from(&["peek"]));
    }

    #[test]
    fn reorder_empty() {
        let args: Vec<String> = vec![];
        assert_eq!(reorder_cli_args(&args), Vec::<String>::new());
    }

    // --- End-to-end parse tests with reorder ---

    #[test]
    fn parse_kind_after_files_with_reorder() {
        let args = args_from(&["peek", "my_func", "src/", "-k", "function"]);
        let cli = Cli::try_parse_from(reorder_cli_args(&args)).unwrap();
        assert_eq!(cli.pattern(), Some("my_func"));
        assert_eq!(cli.files(), &["src/".to_string()]);
        assert_eq!(cli.kinds(), vec![DefKind::Function]);
    }

    #[test]
    fn parse_hidden_after_files_with_reorder() {
        let args = args_from(&["peek", "my_func", "src/", "--hidden"]);
        let cli = Cli::try_parse_from(reorder_cli_args(&args)).unwrap();
        assert_eq!(cli.pattern(), Some("my_func"));
        assert_eq!(cli.files(), &["src/".to_string()]);
        assert!(cli.hidden());
    }

    #[test]
    fn parse_mixed_options_after_files_with_reorder() {
        let args = args_from(&[
            "peek", "my_func", "src/", "-k", "function", "--hidden", "-g", "*.rs",
        ]);
        let cli = Cli::try_parse_from(reorder_cli_args(&args)).unwrap();
        assert_eq!(cli.pattern(), Some("my_func"));
        assert_eq!(cli.files(), &["src/".to_string()]);
        assert_eq!(cli.kinds(), vec![DefKind::Function]);
        assert!(cli.hidden());
        assert_eq!(cli.globs(), &["*.rs".to_string()]);
    }

    #[test]
    fn parse_glob_after_files_with_reorder() {
        let args = args_from(&["peek", "my_func", "src/", "-g", "*.rs", "-g", "!*.test.rs"]);
        let cli = Cli::try_parse_from(reorder_cli_args(&args)).unwrap();
        assert_eq!(cli.pattern(), Some("my_func"));
        assert_eq!(cli.files(), &["src/".to_string()]);
        assert_eq!(cli.globs(), &["*.rs".to_string(), "!*.test.rs".to_string()]);
    }

    // --- dash-prefixed option values ---

    #[test]
    fn reorder_glob_dash_prefixed_value() {
        let args = args_from(&["peek", "my_func", "src/", "-g", "-*.test.rs"]);
        assert_eq!(
            reorder_cli_args(&args),
            args_from(&["peek", "-g=-*.test.rs", "my_func", "src/"])
        );
    }

    #[test]
    fn reorder_regexp_dash_prefixed_value() {
        let args = args_from(&["peek", "src/", "-e", "-pattern"]);
        assert_eq!(
            reorder_cli_args(&args),
            args_from(&["peek", "-e=-pattern", "src/"])
        );
    }

    #[test]
    fn reorder_long_glob_dash_prefixed_value() {
        let args = args_from(&["peek", "my_func", "src/", "--glob", "-*.test.rs"]);
        assert_eq!(
            reorder_cli_args(&args),
            args_from(&["peek", "--glob=-*.test.rs", "my_func", "src/"])
        );
    }

    #[test]
    fn reorder_long_regexp_dash_prefixed_value() {
        let args = args_from(&["peek", "src/", "--regexp", "-pattern"]);
        assert_eq!(
            reorder_cli_args(&args),
            args_from(&["peek", "--regexp=-pattern", "src/"])
        );
    }

    #[test]
    fn reorder_does_not_grab_known_flag_as_value() {
        // "-k" is a known flag, should NOT be grabbed as -g's value
        let args = args_from(&["peek", "my_func", "src/", "-g", "-k", "function"]);
        assert_eq!(
            reorder_cli_args(&args),
            args_from(&["peek", "-g", "-k", "function", "my_func", "src/"])
        );
    }

    #[test]
    fn reorder_does_not_grab_known_boolean_flag_as_value() {
        // "-i" is a known boolean flag, should NOT be grabbed as -g's value
        let args = args_from(&["peek", "my_func", "src/", "-g", "-i"]);
        assert_eq!(
            reorder_cli_args(&args),
            args_from(&["peek", "-g", "-i", "my_func", "src/"])
        );
    }

    #[test]
    fn reorder_value_flag_at_end_preserves_position() {
        // -k at the end with no value should stay after positionals,
        // not move before them where clap would consume a positional as -k's value.
        let args = args_from(&["peek", "my_func", "src/", "-k"]);
        assert_eq!(
            reorder_cli_args(&args),
            args_from(&["peek", "my_func", "src/", "-k"])
        );
    }

    #[test]
    fn reorder_value_long_flag_at_end_preserves_position() {
        // --kind at the end with no value should stay after positionals.
        let args = args_from(&["peek", "my_func", "src/", "--kind"]);
        assert_eq!(
            reorder_cli_args(&args),
            args_from(&["peek", "my_func", "src/", "--kind"])
        );
    }

    #[test]
    fn reorder_end_to_end_dash_prefixed_glob() {
        let args = args_from(&["peek", "my_func", "src/", "-g", "-*.test.rs"]);
        let cli = Cli::try_parse_from(reorder_cli_args(&args)).unwrap();
        assert_eq!(cli.pattern(), Some("my_func"));
        assert_eq!(cli.files(), &["src/".to_string()]);
        assert_eq!(cli.globs(), &["-*.test.rs".to_string()]);
    }

    #[test]
    fn reorder_end_to_end_dash_prefixed_regexp() {
        let args = args_from(&["peek", "src/", "-e", "-pattern"]);
        let cli = Cli::try_parse_from(reorder_cli_args(&args)).unwrap();
        assert_eq!(cli.collect_patterns(), vec!["-pattern".to_string()]);
        assert_eq!(cli.files(), &["src/".to_string()]);
    }
}
