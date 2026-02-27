use clap::builder::styling::{AnsiColor, Styles};
use clap::{ColorChoice, CommandFactory, FromArgMatches, Parser, Subcommand};
use std::path::{Path, PathBuf};

use normalize::commands;
use normalize::commands::analyze::AnalyzeArgs;
use normalize::commands::analyze::AnalyzeCommand;
use normalize::output::OutputFormatter;
use normalize::serve::{self, ServeArgs};

#[derive(Parser)]
#[command(name = "normalize")]
#[command(about = "Fast code intelligence CLI")]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Run command across all git repos under DIR (1 level deep)
    #[arg(long, global = true, value_name = "DIR")]
    repos: Option<PathBuf>,

    /// Output as JSON
    #[arg(long, global = true)]
    json: bool,

    /// Output as JSON Lines (one object per line)
    #[arg(long, global = true)]
    jsonl: bool,

    /// Filter JSON output with jq expression (implies --json)
    #[arg(long, global = true, value_name = "EXPR")]
    jq: Option<String>,

    /// Human-friendly output with colors and formatting
    #[arg(long, global = true, conflicts_with = "compact")]
    pretty: bool,

    /// Compact output without colors (overrides TTY detection)
    #[arg(long, global = true, conflicts_with = "pretty")]
    compact: bool,

    /// Output JSON schema for the command's return type
    #[arg(long, global = true)]
    output_schema: bool,

    /// Output JSON schema for the command's input arguments
    #[arg(long, global = true)]
    input_schema: bool,

    /// Pass command arguments as JSON (overrides CLI args)
    #[arg(long, global = true, value_name = "JSON")]
    params_json: Option<String>,
}

#[derive(Subcommand)]
enum Commands {
    /// Analyze codebase (health, complexity, security, duplicates, docs)
    Analyze(AnalyzeArgs),

    /// Start a normalize server (MCP, HTTP, LSP)
    Serve(ServeArgs),
}

/// Help output styling.
const HELP_STYLES: Styles = Styles::styled()
    .header(AnsiColor::Green.on_default().bold())
    .usage(AnsiColor::Green.on_default().bold())
    .literal(AnsiColor::Cyan.on_default().bold())
    .placeholder(AnsiColor::Cyan.on_default());

/// Determine color choice for help output.
/// Checks args, config, and NO_COLOR before parsing since --help may exit early.
fn help_color_choice() -> ColorChoice {
    // NO_COLOR standard takes precedence
    if std::env::var("NO_COLOR").is_ok() {
        return ColorChoice::Never;
    }

    let args: Vec<String> = std::env::args().collect();
    let has_compact = args.iter().any(|a| a == "--compact");
    let has_pretty = args.iter().any(|a| a == "--pretty");

    // CLI flags override config
    if has_compact {
        return ColorChoice::Never;
    }
    if has_pretty {
        return ColorChoice::Always;
    }

    // Check config for color preference
    let config = normalize::config::NormalizeConfig::load(Path::new("."));
    match config.pretty.colors {
        Some(normalize::output::ColorMode::Always) => ColorChoice::Always,
        Some(normalize::output::ColorMode::Never) => ColorChoice::Never,
        _ => ColorChoice::Auto,
    }
}

/// Reset SIGPIPE to default behavior so piping to `head` etc. doesn't panic.
#[cfg(unix)]
fn reset_sigpipe() {
    // SAFETY: libc::signal is a standard POSIX function. We reset SIGPIPE to default
    // behavior (terminate on broken pipe) instead of Rust's default (ignore, causing
    // write errors). This prevents panics when output is piped to commands like `head`.
    // No memory safety concerns - just changes signal disposition.
    unsafe {
        libc::signal(libc::SIGPIPE, libc::SIG_DFL);
    }
}

#[cfg(not(unix))]
fn reset_sigpipe() {}

/// Handle --schema flag for Nursery integration.
/// Returns JSON with config_path, format, and schema for NormalizeConfig.
fn handle_schema_flag() -> bool {
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).map(|s| s.as_str()) == Some("--schema") {
        let response = serde_json::json!({
            "config_path": ".normalize/config.toml",
            "format": "toml",
            "schema": schemars::schema_for!(normalize::config::NormalizeConfig)
        });
        println!("{}", serde_json::to_string_pretty(&response).unwrap());
        true
    } else {
        false
    }
}

fn main() {
    reset_sigpipe();

    // Handle --schema for Nursery integration (before clap parsing)
    if handle_schema_flag() {
        return;
    }

    // Route migrated commands through server-less
    if try_server_less() {
        return;
    }

    // Parse with custom styles and color choice
    let cli = Cli::command()
        .styles(HELP_STYLES)
        .color(help_color_choice())
        .get_matches();
    let cli = Cli::from_arg_matches(&cli).expect("clap mismatch");

    // Resolve output format at top level - pretty config is TTY-based, not root-specific
    let config = normalize::config::NormalizeConfig::load(Path::new("."));
    let format = normalize::output::OutputFormat::from_cli(
        cli.json,
        cli.jsonl,
        cli.jq.as_deref(),
        cli.pretty,
        cli.compact,
        &config.pretty,
    );

    // Multi-repo dispatch: run supported commands across all repos under --repos DIR
    if let Some(ref repos_dir) = cli.repos {
        let exit_code = run_multi_repo(repos_dir, &cli, &format);
        std::process::exit(exit_code);
    }

    let exit_code = match cli.command {
        Commands::Analyze(args) => commands::analyze::run(
            args,
            format,
            cli.output_schema,
            cli.input_schema,
            cli.params_json.as_deref(),
        ),
        Commands::Serve(args) => serve::run(args, &format),
    };

    std::process::exit(exit_code);
}

/// Commands migrated to server-less `#[cli]`.
const SERVER_LESS_COMMANDS: &[&str] = &[
    "view",
    "grep",
    "aliases",
    "context",
    "init",
    "update",
    "translate",
    "daemon",
    "grammars",
    "generate",
    "facts",
    "rules",
    "package",
    "history",
    "sessions",
    "tools",
    "edit",
];

/// Try dispatching through server-less for migrated commands.
/// Returns true if the command was handled, false to fall through to legacy.
fn try_server_less() -> bool {
    let args: Vec<String> = std::env::args().collect();
    let subcmd = match args.get(1) {
        Some(s) => s.as_str(),
        None => return false,
    };

    if !SERVER_LESS_COMMANDS.contains(&subcmd) {
        return false;
    }

    let service = normalize::service::NormalizeService::new();
    match service.cli_run() {
        Ok(()) => true,
        Err(e) => {
            eprintln!("{}", e);
            std::process::exit(1);
        }
    }
}

/// Run a supported command across all repos under `repos_dir`.
fn run_multi_repo(repos_dir: &Path, cli: &Cli, format: &normalize::output::OutputFormat) -> i32 {
    use normalize::multi_repo::{MultiRepoReport, discover_repos};

    let repos = match discover_repos(repos_dir) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("{}", e);
            return 1;
        }
    };

    if repos.is_empty() {
        eprintln!("No git repositories found under {}", repos_dir.display());
        return 0;
    }

    match &cli.command {
        Commands::Analyze(args) => match &args.command {
            Some(AnalyzeCommand::Hotspots {
                recency,
                allow: None,
                ..
            }) => {
                let recency = *recency;
                let report = MultiRepoReport::run(&repos, |root| {
                    let config = normalize::config::NormalizeConfig::load(root);
                    let mut excludes = config.analyze.hotspots_exclude.clone();
                    excludes.extend(commands::analyze::load_allow_file(root, "hotspots-allow"));
                    commands::analyze::hotspots::analyze_hotspots(root, &excludes, recency)
                });
                let has_errors = report.has_errors();
                report.print(format);
                i32::from(has_errors)
            }
            Some(AnalyzeCommand::Ownership { limit }) => {
                let limit = *limit;
                let exclude = args.exclude.clone();
                let report = MultiRepoReport::run(&repos, |root| {
                    commands::analyze::ownership::analyze_ownership(root, limit, &exclude)
                });
                let has_errors = report.has_errors();
                report.print(format);
                i32::from(has_errors)
            }
            Some(AnalyzeCommand::Coupling { min_commits, limit }) => {
                let min_commits = *min_commits;
                let limit = *limit;
                let exclude = args.exclude.clone();
                let report = MultiRepoReport::run(&repos, |root| {
                    commands::analyze::coupling::analyze_coupling(
                        root,
                        min_commits,
                        limit,
                        &exclude,
                    )
                });
                let has_errors = report.has_errors();
                report.print(format);
                i32::from(has_errors)
            }
            Some(AnalyzeCommand::Activity { window, windows }) => {
                let window = window.clone();
                let windows = *windows;
                match commands::analyze::activity::analyze_activity(&repos, &window, windows) {
                    Ok(report) => {
                        report.print(format);
                        0
                    }
                    Err(e) => {
                        eprintln!("{}", e);
                        1
                    }
                }
            }
            Some(AnalyzeCommand::Contributors) => {
                match commands::analyze::contributors::analyze_contributors(&repos) {
                    Ok(report) => {
                        report.print(format);
                        0
                    }
                    Err(e) => {
                        eprintln!("{}", e);
                        1
                    }
                }
            }
            Some(AnalyzeCommand::RepoCoupling {
                window,
                min_windows,
            }) => {
                let window = *window;
                let min_windows = *min_windows;
                match commands::analyze::repo_coupling::analyze_repo_coupling(
                    &repos,
                    window,
                    min_windows,
                ) {
                    Ok(report) => {
                        report.print(format);
                        0
                    }
                    Err(e) => {
                        eprintln!("{}", e);
                        1
                    }
                }
            }
            _ => {
                eprintln!(
                    "error: --repos is currently supported for: analyze hotspots, analyze ownership, analyze coupling, analyze contributors, analyze repo-coupling, analyze activity"
                );
                1
            }
        },
        _ => {
            eprintln!(
                "error: --repos is currently supported for: analyze hotspots, analyze ownership, analyze coupling, analyze contributors, analyze repo-coupling, analyze activity"
            );
            1
        }
    }
}
