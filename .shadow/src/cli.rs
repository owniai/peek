use crate::model::DefKind;
use clap::Parser;

/// Short flags that take a value (all others are boolean).
/// MUST stay in sync with Cli struct fields that have #[arg(short, long)] and take a value.
const VALUE_SHORT_FLAGS: &[char] = &['k', 'g', 'd'];
/// Long flags that take a value (all others are boolean).
/// MUST stay in sync with Cli struct fields that have #[arg(short, long)] and take a value.
const VALUE_LONG_FLAGS: &[&str] = &["kind", "glob", "max-depth"];

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
/// Limitation: option values starting with `-` must use `--flag=value` syntax,
/// as bare `-` prefixed tokens are always treated as options during reordering.
pub fn reorder_cli_args(args: &[String]) -> Vec<String> // L23-87

#[derive(Parser)]
#[command(name = "peek", about = "Search code definitions by name", version)]
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

    /// Definition name to search for (supports exact and fuzzy matching)
    pattern: String,

    /// Files or directories to search in (default: current directory)
    #[arg(trailing_var_arg = true)]
    files: Vec<String>,
}

impl Cli {
    pub fn pattern(&self) -> &str // L158-160
    pub fn files(&self) -> &Vec<String> // L162-164
    pub fn kinds(&self) -> Vec<DefKind> // L166-171
    pub fn no_signature(&self) -> bool // L173-175
    pub fn ignore_case(&self) -> bool // L177-179
    pub fn smart_case(&self) -> bool // L181-183
    pub fn globs(&self) -> &Vec<String> // L185-187
    pub fn kind_tags(&self) -> Option<&Vec<String>> // L189-191
    pub fn files_with_matches(&self) -> bool // L193-195
    pub fn count(&self) -> bool // L197-199
    pub fn hidden(&self) -> bool // L201-203
    pub fn no_ignore(&self) -> bool // L205-207
    pub fn no_messages(&self) -> bool // L209-211
    pub fn json(&self) -> bool // L213-215
    pub fn max_depth(&self) -> Option<usize> // L217-219
    pub fn with_filename(&self) -> bool // L221-223
    pub fn no_filename(&self) -> bool // L225-227
}

// #[cfg(test)] mod tests { ... } // L230-791 (test module)
