mod cache;
mod cli;
mod mcp;
mod model;
mod output;
mod parser;
mod pattern;
mod pipeline;
mod register;
mod registry;

use std::io::IsTerminal;
use std::path::Path;
use std::process::ExitCode;

use clap::Parser;

use crate::cli::{Cli, Commands, DefArgs, OutlineArgs, SharedArgs, reorder_cli_args};
use crate::model::{Category, DefKind};
use crate::output::OutputMode;
use crate::pattern::CaseSensitivity;
use crate::pipeline::SearchOptions;
use crate::registry::{KNOWN_LANGUAGES, ParserRegistry, resolve_language};

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

    match cli.command {
        Commands::Mcp(_) => {
            mcp::serve()?;
            Ok(ExitCode::SUCCESS)
        }
        Commands::Def(def_args) => run_def(def_args),
        Commands::Outline(outline_args) => run_outline(outline_args),
        Commands::Register(reg_args) => register::run_register(&reg_args),
        Commands::Unregister(unreg_args) => register::run_unregister(&unreg_args),
    }
}

fn run_def(def: DefArgs) -> anyhow::Result<ExitCode> {
    let shared = &def.shared;

    let patterns = def.collect_patterns();
    if patterns.is_empty() {
        anyhow::bail!("error: no pattern specified (use positional argument or -e/--regexp)");
    }

    validate_kinds(shared)?;
    let languages = validate_languages(shared)?;

    let case = if def.ignore_case {
        CaseSensitivity::Insensitive
    } else if def.smart_case {
        CaseSensitivity::SmartCase
    } else {
        CaseSensitivity::Sensitive
    };

    let parsed_patterns: Vec<crate::pattern::ParsedPattern> = patterns
        .iter()
        .map(|p| crate::pattern::ParsedPattern::parse(p, case, def.word))
        .collect::<anyhow::Result<Vec<_>>>()?;
    let modes: Vec<crate::parser::MatchMode> =
        parsed_patterns.iter().map(|p| p.mode().clone()).collect();
    let display_name: String = parsed_patterns
        .iter()
        .map(|p| p.display_name())
        .collect::<Vec<_>>()
        .join("|");

    let search_path_strings = def.files();
    let (search_paths, display_path) = resolve_search_paths(&search_path_strings)?;

    let registry = ParserRegistry::default_registry();
    let search_options = build_search_options(shared);

    let no_filename =
        should_suppress_filename(&search_paths, shared.with_filename, shared.no_filename);
    let heading = resolve_heading(shared);

    let output_mode = if def.files_with_matches {
        OutputMode::FilesOnly
    } else if def.count {
        OutputMode::Count { no_filename }
    } else {
        OutputMode::Normal {
            survey: false,
            no_signature: shared.no_signature,
            no_filename,
            heading,
        }
    };

    execute_search(
        &modes,
        &display_name,
        &display_path,
        &shared.kinds(),
        &search_paths,
        &shared.glob,
        &languages,
        &search_options,
        &registry,
        shared,
        output_mode,
    )
}

fn run_outline(outline: OutlineArgs) -> anyhow::Result<ExitCode> {
    let shared = &outline.shared;

    validate_kinds(shared)?;
    let languages = validate_languages(shared)?;

    let modes = vec![crate::parser::MatchMode::All];
    let display_name = "*".to_string();

    let (search_paths, display_path) = resolve_search_paths(&outline.files)?;

    let registry = ParserRegistry::default_registry();
    let search_options = build_search_options(shared);

    let no_filename =
        should_suppress_filename(&search_paths, shared.with_filename, shared.no_filename);
    let heading = resolve_heading(shared);

    let output_mode = OutputMode::Normal {
        survey: true,
        no_signature: shared.no_signature,
        no_filename,
        heading,
    };

    execute_search(
        &modes,
        &display_name,
        &display_path,
        &shared.kinds(),
        &search_paths,
        &shared.glob,
        &languages,
        &search_options,
        &registry,
        shared,
        output_mode,
    )
}

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

fn validate_kinds(shared: &SharedArgs) -> anyhow::Result<()> {
    if let Some(tags) = &shared.kind {
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
    Ok(())
}

fn validate_languages(shared: &SharedArgs) -> anyhow::Result<Vec<String>> {
    let languages = shared.language.as_ref().cloned().unwrap_or_default();
    if let Some(tags) = &shared.language {
        let unknown: Vec<&str> = tags
            .iter()
            .filter(|t| resolve_language(t).is_none())
            .map(|s| s.as_str())
            .collect();
        if !unknown.is_empty() {
            anyhow::bail!(
                "unknown language(s): {}. Valid languages: {}",
                unknown.join(", "),
                KNOWN_LANGUAGES.join(", ")
            );
        }
    }
    Ok(languages)
}

fn resolve_search_paths(files: &[String]) -> anyhow::Result<(Vec<&Path>, String)> {
    let search_paths: Vec<&Path> = if files.is_empty() {
        vec![Path::new(".")]
    } else {
        files.iter().map(|s| Path::new(s.as_str())).collect()
    };

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

    let display_path = if files.is_empty() {
        ".".to_string()
    } else {
        files.join(", ")
    };

    Ok((search_paths, display_path))
}

fn build_search_options(shared: &SharedArgs) -> SearchOptions {
    SearchOptions {
        hidden: shared.hidden,
        no_ignore: shared.no_ignore,
        max_depth: shared.max_depth,
        max_scope_depth: shared.max_scope_depth,
        project_root: shared
            .project_root
            .as_deref()
            .map(|p| output::absolutize(Path::new(p))),
    }
}

fn resolve_heading(shared: &SharedArgs) -> bool {
    if shared.heading {
        true
    } else if shared.no_heading {
        false
    } else {
        std::io::stdout().is_terminal()
    }
}

#[allow(clippy::too_many_arguments)]
fn execute_search(
    modes: &[crate::parser::MatchMode],
    display_name: &str,
    display_path: &str,
    kinds_for_search: &[DefKind],
    search_paths: &[&Path],
    globs: &[String],
    languages: &[String],
    search_options: &SearchOptions,
    registry: &ParserRegistry,
    shared: &SharedArgs,
    output_mode: OutputMode,
) -> anyhow::Result<ExitCode> {
    let result = pipeline::search(
        modes,
        kinds_for_search,
        search_paths,
        globs,
        languages,
        search_options,
        registry,
    )?;

    let output = if shared.json {
        output::format_json_output(&result, output_mode)
    } else {
        output::format_output(display_name, display_path, &result, output_mode)
    };
    if !output.is_empty() {
        use std::io::Write;
        let stdout = std::io::stdout();
        let mut handle = stdout.lock();
        writeln!(handle, "{}", output)?;
    }

    // Report non-fatal errors to stderr (suppressed by --no-messages)
    let mut stderr = std::io::stderr().lock();
    output::write_errors(&mut stderr, &result, shared.no_messages);

    if !result.read_errors.is_empty() || !result.parse_failures.is_empty() {
        Ok(ExitCode::from(2))
    } else if result.definitions.is_empty() {
        Ok(ExitCode::from(1))
    } else {
        Ok(ExitCode::SUCCESS)
    }
}
