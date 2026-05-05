mod cache;
mod cli;
mod model;
mod output;
mod parser;
mod pattern;
mod pipeline;
mod registry;

use std::io::IsTerminal;
use std::path::Path;
use std::process::ExitCode;

use clap::Parser;

use crate::cli::{Cli, reorder_cli_args};
use crate::model::{Category, DefKind};
use crate::output::OutputMode;
use crate::pattern::CaseSensitivity;
use crate::pipeline::{SearchOptions, SearchResult};
use crate::registry::ParserRegistry;

fn main() -> ExitCode {
    match try_main() {
        Ok(code) => code,
        Err(e) => {
            if is_broken_pipe(&e) {
                return ExitCode::SUCCESS;
            }
            eprintln!("peek: {:#}", e);
            ExitCode::from(2)
        }
    }
}

fn is_broken_pipe(err: &anyhow::Error) -> bool {
    err.chain().any(|e| {
        e.downcast_ref::<std::io::Error>()
            .is_some_and(|io_err| io_err.kind() == std::io::ErrorKind::BrokenPipe)
    })
}

/// Determine whether to suppress filename in output, following ripgrep conventions:
/// - Single file (not directory) search: suppress by default
/// - Directory/multi-path search: show by default
/// - `--with-filename` / `-H`: force show (overrides default and --no-filename)
/// - `--no-filename` / `-I`: force suppress (overrides default)
fn should_suppress_filename(paths: &[&Path], with_filename: bool, no_filename: bool) -> bool {
    if no_filename {
        return true;
    }
    if with_filename {
        return false;
    }
    // Default: suppress when searching a single regular file
    if paths.len() == 1 {
        paths[0].is_file()
    } else {
        false
    }
}

fn try_main() -> anyhow::Result<ExitCode> {
    let args: Vec<String> = std::env::args().collect();
    let cli = Cli::parse_from(reorder_cli_args(&args));

    let files = cli.files();
    let kinds = cli.kinds();
    let no_signature = cli.no_signature();
    let globs = cli.globs();

    // Collect patterns from positional arg and -e/--regexp flags
    let patterns = cli.collect_patterns();
    let survey = cli.is_survey();
    if patterns.is_empty() && !survey {
        anyhow::bail!("error: no pattern specified (use positional argument or -e/--regexp)");
    }

    // Validate --kind tags (accepts both sub-kinds and categories)
    if let Some(tags) = cli.kind_tags() {
        let unknown: Vec<&str> = tags
            .iter()
            .filter(|t| DefKind::from_tag(t).is_none() && Category::from_tag(t).is_none())
            .map(|s| s.as_str())
            .collect();
        if !unknown.is_empty() {
            let mut valid: Vec<&str> = DefKind::all().iter().map(|k| k.display_tag()).collect();
            valid.extend(Category::all().iter().map(|c| c.display_tag()));
            anyhow::bail!(
                "unknown definition type(s): {}. Valid types: {}",
                unknown.join(", "),
                valid.join(", ")
            );
        }
    }

    // Determine case sensitivity from CLI flags
    let case = if cli.ignore_case() {
        CaseSensitivity::Insensitive
    } else if cli.smart_case() {
        CaseSensitivity::SmartCase
    } else {
        CaseSensitivity::Sensitive
    };

    // Parse each pattern independently, or use survey mode
    let (modes, display_name) = if survey {
        (vec![crate::parser::MatchMode::All], "*".to_string())
    } else {
        let parsed_patterns: Vec<crate::pattern::ParsedPattern> = patterns
            .iter()
            .map(|p| crate::pattern::ParsedPattern::parse(p, case, cli.word()))
            .collect::<anyhow::Result<Vec<_>>>()?;
        let ms: Vec<crate::parser::MatchMode> =
            parsed_patterns.iter().map(|p| p.mode().clone()).collect();
        let dn: String = parsed_patterns
            .iter()
            .map(|p| p.display_name())
            .collect::<Vec<_>>()
            .join("|");
        (ms, dn)
    };

    // Determine search paths (default to current directory)
    let search_paths: Vec<&Path> = if files.is_empty() {
        vec![Path::new(".")]
    } else {
        files.iter().map(|s| Path::new(s.as_str())).collect()
    };

    // Validate all paths exist before searching
    let invalid: Vec<&Path> = search_paths
        .iter()
        .filter(|p| !p.exists())
        .copied()
        .collect();
    if !invalid.is_empty() {
        let paths: Vec<String> = invalid.iter().map(|p| p.display().to_string()).collect();
        anyhow::bail!(
            "path(s) do not exist or are not readable: {}",
            paths.join(", ")
        );
    }

    let registry = ParserRegistry::default_registry();

    let search_options = SearchOptions {
        hidden: cli.hidden(),
        no_ignore: cli.no_ignore(),
        max_depth: cli.max_depth(),
        max_scope_depth: cli.max_scope_depth(),
    };

    let output_mode = if cli.files_with_matches() {
        OutputMode::FilesOnly
    } else if cli.count() {
        OutputMode::Count {
            no_filename: should_suppress_filename(
                &search_paths,
                cli.with_filename(),
                cli.no_filename(),
            ),
        }
    } else {
        OutputMode::Normal {
            survey,
            no_signature,
            no_filename: should_suppress_filename(
                &search_paths,
                cli.with_filename(),
                cli.no_filename(),
            ),
            heading: if cli.heading() {
                true
            } else if cli.no_heading() {
                false
            } else {
                std::io::stdout().is_terminal()
            },
        }
    };

    // Single search call for all paths (WalkBuilder::add() handles multi-path internally).
    let result = pipeline::search(
        &modes,
        &kinds,
        &search_paths,
        globs,
        &search_options,
        &registry,
    )?;

    let mut definitions = result.definitions;
    definitions.sort_by(|a, b| a.file.cmp(&b.file));

    let merged_result = SearchResult {
        definitions,
        read_errors: result.read_errors,
        parse_failures: result.parse_failures,
    };

    let display_path = if files.is_empty() {
        ".".to_string()
    } else {
        files.join(", ")
    };

    let output = if cli.json() {
        output::format_json_output(&merged_result, output_mode)
    } else {
        output::format_output(&display_name, &display_path, &merged_result, output_mode)
    };
    if !output.is_empty() {
        use std::io::Write;
        let stdout = std::io::stdout();
        let mut handle = stdout.lock();
        writeln!(handle, "{}", output)?;
    }

    // Report non-fatal errors to stderr (suppressed by --no-messages)
    let mut stderr = std::io::stderr().lock();
    output::write_errors(&mut stderr, &merged_result, cli.no_messages());

    if !merged_result.read_errors.is_empty() || !merged_result.parse_failures.is_empty() {
        Ok(ExitCode::from(2))
    } else if merged_result.definitions.is_empty() {
        Ok(ExitCode::from(1))
    } else {
        Ok(ExitCode::SUCCESS)
    }
}
