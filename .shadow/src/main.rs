mod cache;
mod cli;
mod model;
mod output;
mod parser;
mod pattern;
mod pipeline;
mod registry;

use std::path::Path;
use std::process::ExitCode;

use clap::Parser;

use crate::cli::{Cli, reorder_cli_args};
use crate::model::DefKind;
use crate::output::OutputMode;
use crate::pattern::{CaseSensitivity, ParsedPattern};
use crate::pipeline::{SearchOptions, SearchResult};
use crate::registry::ParserRegistry;

fn main() -> ExitCode // L22-33

fn is_broken_pipe(err: &anyhow::Error) -> bool // L35-40

/// Determine whether to suppress filename in output, following ripgrep conventions:
/// - Single file (not directory) search: suppress by default
/// - Directory/multi-path search: show by default
/// - `--with-filename` / `-H`: force show (overrides default and --no-filename)
/// - `--no-filename` / `-I`: force suppress (overrides default)
fn should_suppress_filename(paths: &[&Path], with_filename: bool, no_filename: bool) -> bool // L47-60

fn try_main() -> anyhow::Result<ExitCode> // L62-196
