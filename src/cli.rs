use crate::model::DefKind;
use clap::{Args, Parser, Subcommand};

/// Short flags that take a value (all others are boolean).
/// MUST stay in sync with struct fields that have #[arg(short, long)] and take a value.
const VALUE_SHORT_FLAGS: &[char] = &['k', 'g', 'd', 'D', 'e'];
/// Long flags that take a value (all others are boolean).
/// MUST stay in sync with struct fields that have #[arg(long)] and take a value.
const VALUE_LONG_FLAGS: &[&str] = &[
    "kind",
    "glob",
    "max-depth",
    "max-scope-depth",
    "regexp",
    "language",
    "project-root",
    "target",
];
/// Short flags that are boolean (no value).
/// MUST stay in sync with struct fields that have #[arg(short)] and no value.
const BOOLEAN_SHORT_FLAGS: &[char] = &['i', 'S', 'l', 'c', 'w', 'H', 'I', 'M'];
/// Long flags that are boolean (no value).
/// MUST stay in sync with struct fields that have #[arg(long)] and no value.
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
    "heading",
    "no-heading",
    "version",
    "help",
    "global",
    "local",
    "list-targets",
];

/// Subcommand names recognized by reorder_cli_args.
/// MUST stay in sync with the `Commands` enum variants.
const SUBCOMMANDS: &[&str] = &["def", "outline", "mcp", "register", "unregister"];

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
/// Skips the subcommand name at args[1] (def/outline/mcp) — only reorders
/// args after the subcommand. This allows `peek def my_func src/ -k function`
/// to be reordered to `peek def -k function my_func src/`.
pub fn reorder_cli_args(args: &[String]) -> Vec<String> {
    if args.len() <= 1 {
        return args.to_vec();
    }

    let program = &args[0];

    // Detect subcommand at args[1] — skip it during reordering
    let subcommand = args.get(1).filter(|a| SUBCOMMANDS.contains(&a.as_str()));
    let start = if subcommand.is_some() { 2 } else { 1 };

    let mut opts = Vec::new();
    let mut positionals = Vec::new();
    let mut i = start;

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

    let mut result = Vec::with_capacity(args.len());
    result.push(program.clone());
    if let Some(sc) = subcommand {
        result.push(sc.clone());
    }
    result.extend(opts);
    result.extend(positionals);
    result
}

// ---------------------------------------------------------------------------
// CLI type definitions
// ---------------------------------------------------------------------------

#[derive(Parser)]
#[command(
    name = "peek",
    bin_name = "peek",
    about = "Search code definitions by name",
    version
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Search for definitions by name
    Def(DefArgs),
    /// List all definitions in files or directories
    Outline(OutlineArgs),
    /// Start MCP server for AI agent integration
    Mcp(McpArgs),
    /// Register peek MCP server with an AI coding platform
    Register(RegisterArgs),
    /// Unregister peek MCP server from an AI coding platform
    Unregister(UnregisterArgs),
}

/// Options shared by def and outline subcommands.
#[derive(Args, Debug)]
pub struct SharedArgs {
    /// Definition types or categories to search for (comma-separated).
    /// Categories expand to all members: shape(class,struct,enum,union,record,object,actor,extension_type),
    /// callable(function,method,constructor,getter,setter,operator,operator_declaration,destructor,subscript),
    /// value(const,event,field,property,var,variant), contract(interface,protocol,trait,extension,mixin,delegate,ambient).
    /// Standalone kinds: alias, module, macro, namespace, package, annotation, concept, associated_type.
    #[arg(short = 'k', long = "kind", value_delimiter = ',')]
    pub kind: Option<Vec<String>>,

    /// Search hidden files and directories
    #[arg(long = "hidden")]
    pub hidden: bool,

    /// Do not respect .gitignore and .ignore rules
    #[arg(long = "no-ignore")]
    pub no_ignore: bool,

    /// Maximum directory traversal depth
    #[arg(short = 'd', long = "max-depth")]
    pub max_depth: Option<usize>,

    /// Glob patterns to filter files (e.g., -g '*.rs', -g '!*.test.rs')
    /// Later globs override earlier globs; '!' prefix negates.
    #[arg(short = 'g', long = "glob")]
    pub glob: Vec<String>,

    /// Only search files of the specified languages (comma-separated).
    /// Accepts canonical names (rust, python) and aliases (js=javascript,
    /// ts=typescript, cpp/c++=cplusplus, cs/c#=csharp).
    #[arg(long = "language", value_delimiter = ',')]
    pub language: Option<Vec<String>>,

    /// Override project root directory. When set, cache is stored under
    /// <root>/.peek-cache/ and relative search paths resolve against this root.
    /// Without this flag, project root is auto-detected from .peek-cache/ in cwd.
    #[arg(long = "project-root")]
    pub project_root: Option<String>,

    /// Show results in JSON format (envelope format compatible with ripgrep --json)
    #[arg(long = "json")]
    pub json: bool,

    /// Suppress signature output
    #[arg(long = "no-signature")]
    pub no_signature: bool,

    /// Always show file path with results (overrides --no-filename)
    #[arg(short = 'H', long = "with-filename", conflicts_with = "no_filename")]
    pub with_filename: bool,

    /// Never show file path with results (overrides --with-filename)
    #[arg(short = 'I', long = "no-filename", conflicts_with = "with_filename")]
    pub no_filename: bool,

    /// Group matches by file, showing the file path once as a heading
    #[arg(long = "heading", conflicts_with = "no_heading")]
    pub heading: bool,

    /// Show file path prefix on every match (default when piped)
    #[arg(long = "no-heading", conflicts_with = "heading")]
    pub no_heading: bool,

    /// Maximum scope path segment depth (1 = top-level only)
    #[arg(short = 'D', long = "max-scope-depth")]
    pub max_scope_depth: Option<usize>,

    /// Suppress non-fatal error messages (traversal, read, and parse errors)
    #[arg(short = 'M', long = "no-messages")]
    pub no_messages: bool,
}

/// Search for definitions by name.
#[derive(Args, Debug)]
pub struct DefArgs {
    #[command(flatten)]
    pub shared: SharedArgs,

    /// Search patterns (can be specified multiple times)
    #[arg(short = 'e', long = "regexp")]
    pub regexp: Vec<String>,

    /// Ignore case distinctions in the pattern
    #[arg(short = 'i', long = "ignore-case")]
    pub ignore_case: bool,

    /// Smart case: ignore case unless the pattern contains uppercase letters
    #[arg(short = 'S', long = "smart-case", conflicts_with = "ignore_case")]
    pub smart_case: bool,

    /// Only match whole words (surround pattern with word boundaries)
    #[arg(short = 'w', long = "word-regexp")]
    pub word: bool,

    /// Only show file paths with matches, not the matched definitions
    #[arg(short = 'l', long = "files-with-matches", conflicts_with = "count")]
    pub files_with_matches: bool,

    /// Show count of matching definitions per file instead of the definitions
    #[arg(short = 'c', long = "count", conflicts_with = "files_with_matches")]
    pub count: bool,

    /// Definition name to search for (supports exact and fuzzy matching)
    /// Optional when -e/--regexp is used.
    pub pattern: Option<String>,

    /// Files or directories to search in (default: current directory)
    #[arg(trailing_var_arg = true)]
    pub files: Vec<String>,
}

/// List all definitions in files or directories.
#[derive(Args, Debug)]
pub struct OutlineArgs {
    #[command(flatten)]
    pub shared: SharedArgs,

    /// Files or directories to outline (default: current directory)
    #[arg(trailing_var_arg = true)]
    pub files: Vec<String>,
}

/// Start MCP server for AI agent integration.
#[derive(Args, Debug)]
pub struct McpArgs {}

/// Register peek MCP server with an AI coding platform.
#[derive(Args, Debug)]
pub struct RegisterArgs {
    /// Target platform (claude, cursor, codex)
    #[arg(long)]
    pub target: Option<String>,

    /// Register globally (user-level config)
    #[arg(long, conflicts_with = "local")]
    pub global: bool,

    /// Register locally (project-level config)
    #[arg(long, conflicts_with = "global")]
    pub local: bool,

    /// List all supported platforms
    #[arg(long)]
    pub list_targets: bool,
}

/// Unregister peek MCP server from an AI coding platform.
#[derive(Args, Debug)]
pub struct UnregisterArgs {
    /// Target platform (claude, cursor, codex)
    #[arg(long)]
    pub target: String,

    /// Unregister globally (user-level config)
    #[arg(long, conflicts_with = "local")]
    pub global: bool,

    /// Unregister locally (project-level config)
    #[arg(long, conflicts_with = "global")]
    pub local: bool,
}

// ---------------------------------------------------------------------------
// Impl blocks
// ---------------------------------------------------------------------------

impl SharedArgs {
    pub fn kinds(&self) -> Vec<DefKind> {
        match &self.kind {
            Some(tags) => tags
                .iter()
                .flat_map(|t| DefKind::kinds_from_tag(t))
                .collect(),
            None => DefKind::all().to_vec(),
        }
    }
}

impl DefArgs {
    /// Collect search patterns: -e flags take precedence; otherwise the
    /// positional pattern argument. Returns empty vec when neither is given.
    pub fn collect_patterns(&self) -> Vec<String> {
        if !self.regexp.is_empty() {
            return self.regexp.clone();
        }
        match &self.pattern {
            Some(p) => vec![p.clone()],
            None => vec![],
        }
    }

    /// Collect file/directory paths. When -e is provided the positional
    /// pattern argument (if any) is treated as a path (ripgrep convention).
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
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_def(args: &[&str]) -> DefArgs {
        let mut full: Vec<&str> = vec!["peek", "def"];
        full.extend_from_slice(args);
        let cli = Cli::try_parse_from(full).unwrap();
        match cli.command {
            Commands::Def(def) => def,
            other => panic!("expected Def, got {:?}", other),
        }
    }

    fn parse_outline(args: &[&str]) -> OutlineArgs {
        let mut full: Vec<&str> = vec!["peek", "outline"];
        full.extend_from_slice(args);
        let cli = Cli::try_parse_from(full).unwrap();
        match cli.command {
            Commands::Outline(outline) => outline,
            other => panic!("expected Outline, got {:?}", other),
        }
    }

    fn args_from(raw: &[&str]) -> Vec<String> {
        raw.iter().map(|s| s.to_string()).collect()
    }

    // --- Subcommand routing ---

    #[test]
    fn no_subcommand_errors() {
        assert!(Cli::try_parse_from(["peek"]).is_err());
    }

    #[test]
    fn def_subcommand_no_args() {
        let def = parse_def(&[]);
        assert!(def.pattern.is_none());
        assert!(def.regexp.is_empty());
        assert!(def.files.is_empty());
    }

    #[test]
    fn outline_subcommand_no_args() {
        let outline = parse_outline(&[]);
        assert!(outline.files.is_empty());
    }

    #[test]
    fn mcp_subcommand_no_args() {
        let full: Vec<&str> = vec!["peek", "mcp"];
        let cli = Cli::try_parse_from(full).unwrap();
        assert!(matches!(cli.command, Commands::Mcp(_)));
    }

    // --- def: pattern and files ---

    #[test]
    fn parse_pattern_only() {
        let def = parse_def(&["my_func"]);
        assert_eq!(def.pattern.as_deref(), Some("my_func"));
        assert!(def.files.is_empty());
        assert!(def.shared.kind.is_none());
    }

    #[test]
    fn parse_pattern_with_single_file() {
        let def = parse_def(&["my_func", "src/"]);
        assert_eq!(def.pattern.as_deref(), Some("my_func"));
        assert_eq!(def.files(), &["src/".to_string()]);
    }

    #[test]
    fn parse_pattern_with_multiple_files() {
        let def = parse_def(&["my_func", "main.cpp", "math.cpp"]);
        assert_eq!(def.pattern.as_deref(), Some("my_func"));
        assert_eq!(
            def.files(),
            &["main.cpp".to_string(), "math.cpp".to_string()]
        );
    }

    #[test]
    fn parse_pattern_with_glob() {
        let def = parse_def(&["Config", "*.rs"]);
        assert_eq!(def.pattern.as_deref(), Some("Config"));
        assert_eq!(def.files(), &["*.rs".to_string()]);
    }

    #[test]
    fn cli_defaults_and_no_signature_flag() {
        let def = parse_def(&["foo"]);
        assert!(!def.shared.no_signature);
        assert!(def.shared.kind.is_none());
        assert!(def.shared.glob.is_empty());
        assert!(!def.ignore_case);
        assert!(!def.smart_case);
        assert_eq!(def.shared.max_depth, None);
        assert!(!def.files_with_matches);
        assert!(!def.count);
        assert!(!def.shared.hidden);
        assert!(!def.shared.no_ignore);
        assert!(!def.shared.json);
        assert!(!def.shared.no_messages);
        assert!(!def.shared.with_filename);
        assert!(!def.shared.no_filename);
        assert!(!def.word);
        assert!(def.regexp.is_empty());
        assert!(!def.shared.heading);
        assert!(!def.shared.no_heading);

        let def = parse_def(&["--no-signature", "foo"]);
        assert!(def.shared.no_signature);
    }

    #[test]
    fn options_after_pattern_still_recognized() {
        let def = parse_def(&["my_func", "-k", "function", "src/"]);
        assert_eq!(def.pattern.as_deref(), Some("my_func"));
        assert_eq!(def.files(), &["src/".to_string()]);
        assert_eq!(
            def.shared.kinds(),
            vec![DefKind::Function, DefKind::FunctionDeclaration]
        );
    }

    // --- glob flags ---

    #[test]
    fn glob_flag_single() {
        let def = parse_def(&["-g", "*.rs", "Config"]);
        assert_eq!(def.pattern.as_deref(), Some("Config"));
        assert_eq!(def.shared.glob, &["*.rs".to_string()]);
    }

    #[test]
    fn glob_flag_long() {
        let def = parse_def(&["--glob", "*.rs", "Config"]);
        assert_eq!(def.pattern.as_deref(), Some("Config"));
        assert_eq!(def.shared.glob, &["*.rs".to_string()]);
    }

    #[test]
    fn glob_flag_multiple() {
        let def = parse_def(&["-g", "*.rs", "-g", "!*.test.rs", "Config"]);
        assert_eq!(def.pattern.as_deref(), Some("Config"));
        assert_eq!(
            def.shared.glob,
            &["*.rs".to_string(), "!*.test.rs".to_string()]
        );
    }

    #[test]
    fn glob_flag_with_negation() {
        let def = parse_def(&["-g", "!*.generated.rs", "Config"]);
        assert_eq!(def.shared.glob, &["!*.generated.rs".to_string()]);
    }

    #[test]
    fn glob_flag_with_path_and_kind() {
        let def = parse_def(&["-k", "class", "-g", "*.rs", "Config", "src/"]);
        assert_eq!(def.pattern.as_deref(), Some("Config"));
        assert_eq!(
            def.shared.kinds(),
            vec![DefKind::Class, DefKind::ClassDeclaration]
        );
        assert_eq!(def.shared.glob, &["*.rs".to_string()]);
        assert_eq!(def.files(), &["src/".to_string()]);
    }

    // --- Case sensitivity flags ---

    #[test]
    fn parse_ignore_case_short() {
        let def = parse_def(&["-i", "foo"]);
        assert!(def.ignore_case);
        assert!(!def.smart_case);
    }

    #[test]
    fn parse_ignore_case_long() {
        let def = parse_def(&["--ignore-case", "foo"]);
        assert!(def.ignore_case);
    }

    #[test]
    fn parse_smart_case_short() {
        let def = parse_def(&["-S", "foo"]);
        assert!(!def.ignore_case);
        assert!(def.smart_case);
    }

    #[test]
    fn parse_smart_case_long() {
        let def = parse_def(&["--smart-case", "foo"]);
        assert!(def.smart_case);
    }

    #[test]
    fn reject_both_ignore_and_smart_case() {
        assert!(Cli::try_parse_from(["peek", "def", "-i", "-S", "foo"]).is_err());
    }

    // --- Output mode flags ---

    #[test]
    fn files_with_matches_short() {
        let def = parse_def(&["-l", "foo"]);
        assert!(def.files_with_matches);
        assert!(!def.count);
    }

    #[test]
    fn files_with_matches_long() {
        let def = parse_def(&["--files-with-matches", "foo"]);
        assert!(def.files_with_matches);
    }

    #[test]
    fn count_short() {
        let def = parse_def(&["-c", "foo"]);
        assert!(def.count);
        assert!(!def.files_with_matches);
    }

    #[test]
    fn count_long() {
        let def = parse_def(&["--count", "foo"]);
        assert!(def.count);
    }

    #[test]
    fn reject_files_with_matches_and_count() {
        assert!(Cli::try_parse_from(["peek", "def", "-l", "-c", "foo"]).is_err());
    }

    // --- Traversal flags ---

    #[test]
    fn hidden_flag() {
        let def = parse_def(&["--hidden", "foo"]);
        assert!(def.shared.hidden);
    }

    #[test]
    fn no_ignore_flag() {
        let def = parse_def(&["--no-ignore", "foo"]);
        assert!(def.shared.no_ignore);
    }

    #[test]
    fn max_depth_short() {
        let def = parse_def(&["-d", "3", "foo"]);
        assert_eq!(def.shared.max_depth, Some(3));
    }

    #[test]
    fn max_depth_long() {
        let def = parse_def(&["--max-depth", "5", "foo"]);
        assert_eq!(def.shared.max_depth, Some(5));
    }

    #[test]
    fn max_scope_depth_short() {
        let def = parse_def(&["-D", "2", "foo"]);
        assert_eq!(def.shared.max_scope_depth, Some(2));
    }

    #[test]
    fn max_scope_depth_long() {
        let def = parse_def(&["--max-scope-depth", "3", "foo"]);
        assert_eq!(def.shared.max_scope_depth, Some(3));
    }

    #[test]
    fn max_scope_depth_default_is_none() {
        let def = parse_def(&["foo"]);
        assert_eq!(def.shared.max_scope_depth, None);
    }

    // --- --json flag ---

    #[test]
    fn json_flag() {
        let def = parse_def(&["--json", "foo"]);
        assert!(def.shared.json);
    }

    // --- --no-messages flag ---

    #[test]
    fn no_messages_short() {
        let def = parse_def(&["-M", "foo"]);
        assert!(def.shared.no_messages);
    }

    #[test]
    fn no_messages_long() {
        let def = parse_def(&["--no-messages", "foo"]);
        assert!(def.shared.no_messages);
    }

    // --- --with-filename / --no-filename flags ---

    #[test]
    fn with_filename_short() {
        let def = parse_def(&["-H", "foo"]);
        assert!(def.shared.with_filename);
        assert!(!def.shared.no_filename);
    }

    #[test]
    fn with_filename_long() {
        let def = parse_def(&["--with-filename", "foo"]);
        assert!(def.shared.with_filename);
    }

    #[test]
    fn no_filename_short() {
        let def = parse_def(&["-I", "foo"]);
        assert!(!def.shared.with_filename);
        assert!(def.shared.no_filename);
    }

    #[test]
    fn no_filename_long() {
        let def = parse_def(&["--no-filename", "foo"]);
        assert!(def.shared.no_filename);
    }

    #[test]
    fn reject_both_with_and_no_filename() {
        assert!(Cli::try_parse_from(["peek", "def", "-H", "-I", "foo"]).is_err());
    }

    // --- --heading / --no-heading flags ---

    #[test]
    fn heading_long() {
        let def = parse_def(&["--heading", "foo"]);
        assert!(def.shared.heading);
        assert!(!def.shared.no_heading);
    }

    #[test]
    fn no_heading_long() {
        let def = parse_def(&["--no-heading", "foo"]);
        assert!(!def.shared.heading);
        assert!(def.shared.no_heading);
    }

    #[test]
    fn reject_both_heading_and_no_heading() {
        assert!(Cli::try_parse_from(["peek", "def", "--heading", "--no-heading", "foo"]).is_err());
    }

    // --- --word-regexp flag ---

    #[test]
    fn word_regexp_short() {
        let def = parse_def(&["-w", "foo"]);
        assert!(def.word);
    }

    #[test]
    fn word_regexp_long() {
        let def = parse_def(&["--word-regexp", "foo"]);
        assert!(def.word);
    }

    // --- -e/--regexp flag ---

    #[test]
    fn regexp_flag_short() {
        let def = parse_def(&["-e", "foo"]);
        assert_eq!(def.regexp, &["foo".to_string()]);
        assert!(def.pattern.is_none());
    }

    #[test]
    fn regexp_flag_long() {
        let def = parse_def(&["--regexp", "foo"]);
        assert_eq!(def.regexp, &["foo".to_string()]);
    }

    #[test]
    fn regexp_flag_multiple() {
        let def = parse_def(&["-e", "foo", "-e", "bar"]);
        assert_eq!(def.regexp, &["foo".to_string(), "bar".to_string()]);
    }

    #[test]
    fn regexp_with_positional_pattern() {
        let def = parse_def(&["baz", "-e", "foo"]);
        assert_eq!(def.pattern.as_deref(), Some("baz"));
        assert_eq!(def.regexp, &["foo".to_string()]);
    }

    #[test]
    fn collect_patterns_positional_only() {
        let def = parse_def(&["foo"]);
        assert_eq!(def.collect_patterns(), vec!["foo".to_string()]);
    }

    #[test]
    fn collect_patterns_regexp_only() {
        let def = parse_def(&["-e", "foo", "-e", "bar"]);
        assert_eq!(
            def.collect_patterns(),
            vec!["foo".to_string(), "bar".to_string()]
        );
    }

    #[test]
    fn collect_patterns_mixed() {
        // When -e is present, positional pattern is excluded from patterns
        let def = parse_def(&["baz", "-e", "foo", "-e", "bar"]);
        assert_eq!(
            def.collect_patterns(),
            vec!["foo".to_string(), "bar".to_string()]
        );
    }

    #[test]
    fn collect_patterns_empty_when_none() {
        let def = parse_def(&[]);
        assert!(def.collect_patterns().is_empty());
    }

    // --- -e turns positional into path (ripgrep alignment) ---

    #[test]
    fn regexp_turns_positional_into_path() {
        let def = parse_def(&["src/", "-e", "foo"]);
        assert_eq!(def.collect_patterns(), vec!["foo".to_string()]);
        assert_eq!(def.files(), &["src/".to_string()]);
    }

    #[test]
    fn regexp_positional_and_trailing_files_merged() {
        let args = args_from(&["peek", "def", "src/", "lib/", "-e", "foo"]);
        let def = match Cli::try_parse_from(reorder_cli_args(&args))
            .unwrap()
            .command
        {
            Commands::Def(d) => d,
            _ => panic!("expected Def"),
        };
        assert_eq!(def.collect_patterns(), vec!["foo".to_string()]);
        assert_eq!(def.files(), &["src/".to_string(), "lib/".to_string()]);
    }

    #[test]
    fn regexp_without_positional_empty_files() {
        let def = parse_def(&["-e", "foo"]);
        assert_eq!(def.collect_patterns(), vec!["foo".to_string()]);
        assert!(def.files().is_empty());
    }

    #[test]
    fn no_regexp_positional_stays_as_pattern() {
        let def = parse_def(&["foo", "src/"]);
        assert_eq!(def.collect_patterns(), vec!["foo".to_string()]);
        assert_eq!(def.files(), &["src/".to_string()]);
    }

    // --- outline subcommand ---

    #[test]
    fn outline_with_files() {
        let outline = parse_outline(&["src/", "lib/"]);
        assert_eq!(outline.files, &["src/".to_string(), "lib/".to_string()]);
    }

    #[test]
    fn outline_with_kind_flag() {
        let outline = parse_outline(&["-k", "function", "src/"]);
        assert_eq!(
            outline.shared.kind.as_ref().unwrap(),
            &["function".to_string()]
        );
    }

    #[test]
    fn outline_with_json_flag() {
        let outline = parse_outline(&["--json", "src/"]);
        assert!(outline.shared.json);
    }

    // --- reorder_cli_args ---

    #[test]
    fn reorder_preserves_positional_order() {
        let args = args_from(&["peek", "def", "my_func", "src/", "lib/", "-k", "function"]);
        assert_eq!(
            reorder_cli_args(&args),
            args_from(&["peek", "def", "-k", "function", "my_func", "src/", "lib/"])
        );
    }

    #[test]
    fn reorder_skips_subcommand() {
        let args = args_from(&["peek", "outline", "src/", "--hidden"]);
        assert_eq!(
            reorder_cli_args(&args),
            args_from(&["peek", "outline", "--hidden", "src/"])
        );
    }

    // --- End-to-end parse tests with reorder ---

    #[test]
    fn parse_hidden_after_files_with_reorder() {
        let args = args_from(&["peek", "def", "my_func", "src/", "--hidden"]);
        let def = match Cli::try_parse_from(reorder_cli_args(&args))
            .unwrap()
            .command
        {
            Commands::Def(d) => d,
            _ => panic!("expected Def"),
        };
        assert_eq!(def.pattern.as_deref(), Some("my_func"));
        assert_eq!(def.files(), &["src/".to_string()]);
        assert!(def.shared.hidden);
    }

    #[test]
    fn parse_glob_after_files_with_reorder() {
        let args = args_from(&[
            "peek",
            "def",
            "my_func",
            "src/",
            "-g",
            "*.rs",
            "-g",
            "!*.test.rs",
        ]);
        let def = match Cli::try_parse_from(reorder_cli_args(&args))
            .unwrap()
            .command
        {
            Commands::Def(d) => d,
            _ => panic!("expected Def"),
        };
        assert_eq!(def.pattern.as_deref(), Some("my_func"));
        assert_eq!(def.files(), &["src/".to_string()]);
        assert_eq!(
            def.shared.glob,
            &["*.rs".to_string(), "!*.test.rs".to_string()]
        );
    }

    // --- dash-prefixed option values ---

    #[test]
    fn reorder_end_to_end_dash_prefixed_glob() {
        let args = args_from(&["peek", "def", "my_func", "src/", "-g", "-*.test.rs"]);
        let def = match Cli::try_parse_from(reorder_cli_args(&args))
            .unwrap()
            .command
        {
            Commands::Def(d) => d,
            _ => panic!("expected Def"),
        };
        assert_eq!(def.pattern.as_deref(), Some("my_func"));
        assert_eq!(def.files(), &["src/".to_string()]);
        assert_eq!(def.shared.glob, &["-*.test.rs".to_string()]);
    }

    #[test]
    fn reorder_end_to_end_dash_prefixed_regexp() {
        let args = args_from(&["peek", "def", "src/", "-e", "-pattern"]);
        let def = match Cli::try_parse_from(reorder_cli_args(&args))
            .unwrap()
            .command
        {
            Commands::Def(d) => d,
            _ => panic!("expected Def"),
        };
        assert_eq!(def.collect_patterns(), vec!["-pattern".to_string()]);
        assert_eq!(def.files(), &["src/".to_string()]);
    }

    // --- --language flag ---

    #[test]
    fn language_flag_single() {
        let def = parse_def(&["--language", "rust", "foo"]);
        assert_eq!(def.shared.language, Some(vec!["rust".to_string()]));
    }

    #[test]
    fn language_flag_comma_separated() {
        let def = parse_def(&["--language", "rust,python", "foo"]);
        assert_eq!(
            def.shared.language,
            Some(vec!["rust".to_string(), "python".to_string()])
        );
    }

    #[test]
    fn language_flag_with_alias() {
        let def = parse_def(&["--language", "js", "foo"]);
        assert_eq!(def.shared.language, Some(vec!["js".to_string()]));
    }

    #[test]
    fn language_flag_default_is_none() {
        let def = parse_def(&["foo"]);
        assert!(def.shared.language.is_none());
    }

    // --- register subcommand ---

    fn parse_register(args: &[&str]) -> RegisterArgs {
        let mut full: Vec<&str> = vec!["peek", "register"];
        full.extend_from_slice(args);
        let cli = Cli::try_parse_from(full).unwrap();
        match cli.command {
            Commands::Register(r) => r,
            other => panic!("expected Register, got {:?}", other),
        }
    }

    #[test]
    fn register_target_flag() {
        let reg = parse_register(&["--target", "claude"]);
        assert_eq!(reg.target.as_deref(), Some("claude"));
        assert!(!reg.global);
        assert!(!reg.local);
        assert!(!reg.list_targets);
    }

    #[test]
    fn register_global_flag() {
        let reg = parse_register(&["--target", "claude", "--global"]);
        assert!(reg.global);
        assert!(!reg.local);
    }

    #[test]
    fn register_local_flag() {
        let reg = parse_register(&["--target", "claude", "--local"]);
        assert!(!reg.global);
        assert!(reg.local);
    }

    #[test]
    fn register_list_targets_flag() {
        let reg = parse_register(&["--list-targets"]);
        assert!(reg.list_targets);
        assert!(reg.target.is_none());
    }

    #[test]
    fn register_defaults() {
        let reg = parse_register(&[]);
        assert!(reg.target.is_none());
        assert!(!reg.global);
        assert!(!reg.local);
        assert!(!reg.list_targets);
    }

    #[test]
    fn reject_register_global_and_local() {
        assert!(
            Cli::try_parse_from([
                "peek", "register", "--target", "claude", "--global", "--local"
            ])
            .is_err()
        );
    }

    // --- unregister subcommand ---

    fn parse_unregister(args: &[&str]) -> UnregisterArgs {
        let mut full: Vec<&str> = vec!["peek", "unregister"];
        full.extend_from_slice(args);
        let cli = Cli::try_parse_from(full).unwrap();
        match cli.command {
            Commands::Unregister(u) => u,
            other => panic!("expected Unregister, got {:?}", other),
        }
    }

    #[test]
    fn unregister_target_required() {
        assert!(Cli::try_parse_from(["peek", "unregister"]).is_err());
    }

    #[test]
    fn unregister_global_flag() {
        let unreg = parse_unregister(&["--target", "claude", "--global"]);
        assert_eq!(unreg.target, "claude");
        assert!(unreg.global);
        assert!(!unreg.local);
    }

    #[test]
    fn unregister_local_flag() {
        let unreg = parse_unregister(&["--target", "claude", "--local"]);
        assert_eq!(unreg.target, "claude");
        assert!(!unreg.global);
        assert!(unreg.local);
    }

    #[test]
    fn unregister_default_is_global() {
        let unreg = parse_unregister(&["--target", "claude"]);
        assert!(!unreg.global);
        assert!(!unreg.local);
    }

    #[test]
    fn reject_unregister_global_and_local() {
        assert!(
            Cli::try_parse_from([
                "peek",
                "unregister",
                "--target",
                "claude",
                "--global",
                "--local"
            ])
            .is_err()
        );
    }
}
