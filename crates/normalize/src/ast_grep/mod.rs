#![allow(warnings, clippy::all, unexpected_cfgs)]
// Vendored from ast-grep 0.41.0 (MIT)
// Embeds ast-grep as a drop-in `ast-grep`/`sg` replacement.
// Uses normalize-languages' dynamic grammar loading instead of ast-grep-language.

pub(crate) mod config;
pub(crate) mod lang;
pub(crate) mod print;
pub(crate) mod run;
pub(crate) mod scan;
pub(crate) mod utils;

use std::ffi::OsString;
use std::process::ExitCode;

use anyhow::Result;
use clap::{Parser, Subcommand};

use config::ProjectConfig;
use run::{RunArg, run_with_pattern};
use scan::{ScanArg, run_with_config};
use utils::exit_with_error;

const LOGO: &str = r#"
Search and Rewrite code at large scale using AST pattern.
                    __
        ____ ______/ /_      ____ _________  ____
       / __ `/ ___/ __/_____/ __ `/ ___/ _ \/ __ \
      / /_/ (__  ) /_/_____/ /_/ / /  /  __/ /_/ /
      \__,_/____/\__/      \__, /_/   \___/ .___/
                          /____/         /_/
"#;

#[derive(Parser)]
#[clap(author, version, about, long_about = LOGO)]
struct App {
    #[clap(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Run one time search or rewrite in command line. (default command)
    Run(RunArg),
    /// Scan the codebase with rules from sgconfig.yml or specified rule files.
    Scan(ScanArg),
}

/// Run ast-grep with the given arguments (not including argv[0]).
pub fn run_ast_grep(args: impl Iterator<Item = OsString>) -> ExitCode {
    let args: Vec<String> = std::iter::once("sg".to_string())
        .chain(args.filter_map(|s| s.into_string().ok()))
        .collect();

    match main_with_args(args.into_iter()) {
        Ok(code) => code,
        Err(error) => match exit_with_error(error) {
            Ok(code) => code,
            Err(_) => ExitCode::from(2),
        },
    }
}

fn is_command(arg: &str, command: &str) -> bool {
    let arg = arg.split('=').next().unwrap_or(arg);
    if arg.starts_with("--") {
        let arg = arg.trim_start_matches("--");
        arg == command
    } else if arg.starts_with('-') {
        let arg = arg.trim_start_matches('-');
        arg == &command[..1]
    } else {
        false
    }
}

fn try_default_run(args: &[String]) -> Result<Option<RunArg>> {
    // use `run` if there is at lease one pattern arg with no user provided command
    let should_use_default_run_command =
        args.iter().skip(1).any(|p| is_command(p, "pattern")) && args[1].starts_with('-');
    if should_use_default_run_command {
        let arg = RunArg::try_parse_from(args)?;
        Ok(Some(arg))
    } else {
        Ok(None)
    }
}

fn main_with_args(args: impl Iterator<Item = String>) -> Result<ExitCode> {
    let args: Vec<_> = args.collect();
    if let Some(arg) = try_default_run(&args)? {
        return run_with_pattern(arg);
    }
    let app = App::try_parse_from(args)?;
    match app.command {
        Commands::Run(arg) => run_with_pattern(arg),
        Commands::Scan(arg) => {
            let project = ProjectConfig::setup(None)?;
            run_with_config(arg, project)
        }
    }
}
