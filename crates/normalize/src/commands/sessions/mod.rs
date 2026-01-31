//! Sessions command - analyze Claude Code and other agent session logs.

pub mod analyze;
pub mod list;
pub mod plans;
#[cfg(feature = "sessions-web")]
mod serve;
pub mod show;
pub mod stats;

pub use list::cmd_sessions_list;
#[cfg(feature = "sessions-web")]
pub use serve::cmd_sessions_serve;
pub use show::{SessionShowReport, cmd_sessions_show};
pub use stats::cmd_sessions_stats;

use clap::{Args, Subcommand};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use crate::sessions::{FormatRegistry, LogFormat};
use nucleo_matcher::pattern::{CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Config, Matcher};

/// Format an age in seconds to a human-readable string.
pub(crate) fn format_age(secs: u64) -> String {
    if secs < 60 {
        format!("{}s ago", secs)
    } else if secs < 3600 {
        format!("{}m ago", secs / 60)
    } else if secs < 86400 {
        format!("{}h ago", secs / 3600)
    } else {
        format!("{}d ago", secs / 86400)
    }
}

/// Resolve a session identifier to one or more file paths.
/// Tries literal (exact/prefix) matching first, then falls back to fuzzy matching.
pub(crate) fn resolve_session_paths(
    session_id: &str,
    project: Option<&Path>,
    format_name: Option<&str>,
) -> Vec<PathBuf> {
    let literal = resolve_session_paths_literal(session_id, project, format_name);
    if !literal.is_empty() {
        return literal;
    }

    // Fuzzy fallback
    if let Some((path, matched_stem)) = resolve_session_fuzzy(session_id, project, format_name) {
        eprintln!("fuzzy match: {}", matched_stem);
        return vec![path];
    }

    Vec::new()
}

/// Resolve a session identifier using exact match or prefix only.
/// Supports: full path, session ID prefix, glob pattern.
pub(crate) fn resolve_session_paths_literal(
    session_id: &str,
    project: Option<&Path>,
    format_name: Option<&str>,
) -> Vec<PathBuf> {
    let session_path = Path::new(session_id);

    // If it's a full path, use it directly
    if session_path.is_file() {
        return vec![session_path.to_path_buf()];
    }

    // If it looks like a glob pattern, expand it
    if session_id.contains('*') || session_id.contains('?') {
        if let Ok(entries) = glob::glob(session_id) {
            let paths: Vec<_> = entries
                .filter_map(|e| e.ok())
                .filter(|p| p.is_file())
                .collect();
            if !paths.is_empty() {
                return paths;
            }
        }
    }

    // Otherwise, try to find it as a session ID in the format's directory
    let registry = FormatRegistry::new();
    let format: &dyn LogFormat = match format_name {
        Some(name) => match registry.get(name) {
            Some(f) => f,
            None => return Vec::new(),
        },
        None => registry.get("claude").unwrap(),
    };

    let sessions = format.list_sessions(project);

    // Match by session ID prefix (file stem)
    for s in &sessions {
        if let Some(stem) = s.path.file_stem().and_then(|s| s.to_str()) {
            if stem == session_id || stem.starts_with(session_id) {
                return vec![s.path.clone()];
            }
        }
    }

    // No match
    Vec::new()
}

/// Fuzzy-match a session identifier against all session file stems.
/// Returns the best match (highest score) along with the matched stem.
fn resolve_session_fuzzy(
    session_id: &str,
    project: Option<&Path>,
    format_name: Option<&str>,
) -> Option<(PathBuf, String)> {
    let registry = FormatRegistry::new();
    let format: &dyn LogFormat = match format_name {
        Some(name) => registry.get(name)?,
        None => registry.get("claude").unwrap(),
    };

    let sessions = format.list_sessions(project);
    let mut matcher = Matcher::new(Config::DEFAULT);
    let pattern = Pattern::parse(session_id, CaseMatching::Ignore, Normalization::Smart);

    let mut best: Option<(PathBuf, String, u32)> = None;

    for s in &sessions {
        if let Some(stem) = s.path.file_stem().and_then(|os| os.to_str()) {
            let mut buf = Vec::new();
            if let Some(score) =
                pattern.score(nucleo_matcher::Utf32Str::new(stem, &mut buf), &mut matcher)
            {
                if best
                    .as_ref()
                    .is_none_or(|(_, _, best_score)| score > *best_score)
                {
                    best = Some((s.path.clone(), stem.to_string(), score));
                }
            }
        }
    }

    best.map(|(path, stem, _)| (path, stem))
}

/// Helper for default limit
fn default_limit() -> usize {
    20
}

/// Sessions command arguments
#[derive(Args, serde::Deserialize, schemars::JsonSchema)]
pub struct SessionsArgs {
    #[command(subcommand)]
    pub command: Option<SessionsCommand>,

    /// Root directory (defaults to current directory)
    #[arg(short, long, global = true)]
    pub root: Option<PathBuf>,

    /// Force specific format: claude, codex, gemini, moss
    #[arg(long, global = true)]
    pub format: Option<String>,

    /// Limit number of sessions
    #[arg(short, long, default_value = "20", global = true)]
    #[serde(default = "default_limit")]
    pub limit: usize,
}

#[derive(Subcommand, serde::Deserialize, schemars::JsonSchema)]
pub enum SessionsCommand {
    /// List sessions
    List {
        /// Filter sessions by grep pattern (searches prompt/commands)
        #[arg(long)]
        grep: Option<String>,

        /// Filter sessions from the last N days
        #[arg(long)]
        days: Option<u32>,

        /// Filter sessions since date (YYYY-MM-DD)
        #[arg(long)]
        since: Option<String>,

        /// Filter sessions until date (YYYY-MM-DD)
        #[arg(long)]
        until: Option<String>,

        /// Filter by specific project path
        #[arg(long)]
        project: Option<PathBuf>,

        /// Show sessions from all projects (not just current)
        #[arg(long)]
        #[serde(default)]
        all_projects: bool,
    },

    /// Show a specific session
    Show {
        /// Session ID or path
        session: String,

        /// Require exact/prefix match (disable fuzzy fallback)
        #[arg(long)]
        #[serde(default)]
        exact: bool,

        /// Apply jq filter to each JSONL line
        #[arg(long)]
        jq: Option<String>,

        /// Run full analysis instead of summary
        #[arg(short, long)]
        #[serde(default)]
        analyze: bool,

        /// Show full conversation log (default is summary)
        #[arg(long)]
        #[serde(default)]
        full: bool,

        /// Filter messages by role/type: user, assistant, system, tool_use, tool_result, thinking
        #[arg(long)]
        filter: Option<String>,

        /// Search for pattern in filtered messages (requires --filter or shows all matching)
        #[arg(long)]
        grep: Option<String>,

        /// Show only error tool results (implies --filter tool_result)
        #[arg(long)]
        #[serde(default)]
        errors_only: bool,

        /// Extract common word sequences (ngrams) of length N (2-4)
        #[arg(long)]
        ngrams: Option<usize>,

        /// Case-insensitive ngram matching
        #[arg(long)]
        #[serde(default)]
        case_insensitive: bool,
    },

    /// Show aggregate statistics across sessions
    Stats {
        /// Filter sessions by grep pattern
        #[arg(long)]
        grep: Option<String>,

        /// Filter sessions from the last N days
        #[arg(long)]
        days: Option<u32>,

        /// Filter sessions since date (YYYY-MM-DD)
        #[arg(long)]
        since: Option<String>,

        /// Filter sessions until date (YYYY-MM-DD)
        #[arg(long)]
        until: Option<String>,

        /// Filter by specific project path
        #[arg(long)]
        project: Option<PathBuf>,

        /// Show sessions from all projects (not just current)
        #[arg(long)]
        #[serde(default)]
        all_projects: bool,

        /// Group statistics by repository
        #[arg(long)]
        #[serde(default)]
        by_repo: bool,
    },

    /// Start web server for viewing sessions
    #[cfg(feature = "sessions-web")]
    Serve {
        /// Port for web server
        #[arg(long, default_value = "3939")]
        port: u16,
    },

    /// List and view agent plans (from ~/.claude/plans/, etc.)
    Plans {
        /// Plan name to view (omit to list all plans)
        name: Option<String>,
    },
}

/// Print JSON schema for the sessions subcommand's output type.
fn print_sessions_schema(command: &Option<SessionsCommand>) -> i32 {
    use crate::sessions::SessionAnalysis;
    match command {
        Some(SessionsCommand::List { .. }) | None => {
            crate::output::print_output_schema::<list::SessionListReport>();
        }
        Some(SessionsCommand::Show { analyze: true, .. }) | Some(SessionsCommand::Stats { .. }) => {
            crate::output::print_output_schema::<SessionAnalysis>();
        }
        Some(SessionsCommand::Show { .. }) => {
            crate::output::print_output_schema::<show::SessionShowReport>();
        }
        Some(SessionsCommand::Plans { .. }) => {
            crate::output::print_output_schema::<plans::PlansListReport>();
        }
        #[cfg(feature = "sessions-web")]
        Some(SessionsCommand::Serve { .. }) => {
            eprintln!("Serve subcommand does not have a structured output schema");
            return 1;
        }
    }
    0
}

/// Print JSON schema for the command's input arguments.
pub fn print_input_schema() {
    let schema = schemars::schema_for!(SessionsArgs);
    println!(
        "{}",
        serde_json::to_string_pretty(&schema).unwrap_or_default()
    );
}

/// Run the sessions command
pub fn run(
    args: SessionsArgs,
    output_format: &crate::output::OutputFormat,
    output_schema: bool,
    input_schema: bool,
    params_json: Option<&str>,
) -> i32 {
    let json = output_format.is_json();
    if output_schema {
        return print_sessions_schema(&args.command);
    }
    if input_schema {
        print_input_schema();
        return 0;
    }
    // Override args with --params-json if provided
    let args = match params_json {
        Some(json_str) => match serde_json::from_str(json_str) {
            Ok(parsed) => parsed,
            Err(e) => {
                eprintln!("error: invalid --params-json: {}", e);
                return 1;
            }
        },
        None => args,
    };
    match args.command {
        Some(SessionsCommand::List {
            grep,
            days,
            since,
            until,
            project,
            all_projects,
        }) => cmd_sessions_list_filtered(
            args.root.as_deref(),
            args.limit,
            args.format.as_deref(),
            grep.as_deref(),
            days,
            since.as_deref(),
            until.as_deref(),
            project.as_deref(),
            all_projects,
            json,
        ),

        Some(SessionsCommand::Show {
            session,
            exact,
            jq,
            analyze,
            full,
            filter,
            grep,
            errors_only,
            ngrams,
            case_insensitive,
        }) => cmd_sessions_show(
            &session,
            args.root.as_deref(),
            jq.as_deref(),
            args.format.as_deref(),
            analyze,
            full,
            output_format,
            filter.as_deref(),
            grep.as_deref(),
            errors_only,
            ngrams,
            case_insensitive,
            exact,
        ),

        Some(SessionsCommand::Stats {
            grep,
            days,
            since,
            until,
            project,
            all_projects,
            by_repo,
        }) => cmd_sessions_stats(
            args.root.as_deref(),
            args.limit,
            args.format.as_deref(),
            grep.as_deref(),
            days,
            since.as_deref(),
            until.as_deref(),
            project.as_deref(),
            all_projects,
            by_repo,
            json,
            output_format.is_pretty(),
        ),

        #[cfg(feature = "sessions-web")]
        Some(SessionsCommand::Serve { port }) => {
            let rt = tokio::runtime::Runtime::new().expect("Failed to create runtime");
            rt.block_on(cmd_sessions_serve(args.root.as_deref(), port))
        }

        Some(SessionsCommand::Plans { name }) => {
            plans::cmd_plans(name.as_deref(), args.limit, json)
        }

        // Default: list sessions
        None => cmd_sessions_list(
            args.root.as_deref(),
            args.limit,
            args.format.as_deref(),
            None,
            json,
        ),
    }
}

/// List sessions with filtering support
fn cmd_sessions_list_filtered(
    root: Option<&Path>,
    limit: usize,
    format_name: Option<&str>,
    grep: Option<&str>,
    days: Option<u32>,
    since: Option<&str>,
    until: Option<&str>,
    project: Option<&Path>,
    all_projects: bool,
    json: bool,
) -> i32 {
    // For now, delegate to stats module's filtering logic but output as list
    // TODO: Refactor to share filtering between list and stats
    use crate::sessions::{FormatRegistry, LogFormat, SessionFile};
    use std::time::{Duration, SystemTime};

    let registry = FormatRegistry::new();
    let format: &dyn LogFormat = match format_name {
        Some(name) => match registry.get(name) {
            Some(f) => f,
            None => {
                eprintln!("Unknown format: {}", name);
                return 1;
            }
        },
        None => registry.get("claude").unwrap(),
    };

    // Compile grep pattern
    let grep_re = grep.map(|p| regex::Regex::new(p).ok()).flatten();
    if grep.is_some() && grep_re.is_none() {
        eprintln!("Invalid grep pattern: {}", grep.unwrap());
        return 1;
    }

    // Get sessions
    let mut sessions: Vec<SessionFile> = if all_projects {
        stats::list_all_project_sessions(format)
    } else {
        let proj = project.or(root);
        format.list_sessions(proj)
    };

    // Date filtering
    let now = SystemTime::now();
    if let Some(d) = days {
        let since_time = now - Duration::from_secs(d as u64 * 86400);
        sessions.retain(|s| s.mtime >= since_time);
    }
    if let Some(s) = since {
        if let Some(since_time) = stats::parse_date(s) {
            sessions.retain(|s| s.mtime >= since_time);
        }
    }
    if let Some(u) = until {
        if let Some(until_time) = stats::parse_date(u) {
            let until_time = until_time + Duration::from_secs(86400);
            sessions.retain(|s| s.mtime <= until_time);
        }
    }

    // Grep filtering
    if let Some(ref re) = grep_re {
        sessions.retain(|s| session_matches_grep(&s.path, re));
    }

    // Sort and limit
    sessions.sort_by(|a, b| b.mtime.cmp(&a.mtime));
    if limit > 0 {
        sessions.truncate(limit);
    }

    // Output
    if json {
        let paths: Vec<_> = sessions
            .iter()
            .map(|s| s.path.display().to_string())
            .collect();
        println!("{}", serde_json::to_string_pretty(&paths).unwrap());
    } else {
        for s in &sessions {
            println!("{}", s.path.display());
        }
        if !sessions.is_empty() {
            eprintln!("\n{} sessions", sessions.len());
        }
    }

    0
}

/// Check if a session file matches a grep pattern.
/// Searches through the raw JSONL content for any match.
pub(crate) fn session_matches_grep(path: &Path, pattern: &regex::Regex) -> bool {
    let Ok(file) = File::open(path) else {
        return false;
    };
    let reader = BufReader::new(file);
    for line in reader.lines().map_while(Result::ok) {
        if pattern.is_match(&line) {
            return true;
        }
    }
    false
}

/// Get the Claude Code sessions directory for a project.
#[cfg(feature = "sessions-web")]
pub(crate) fn get_sessions_dir(project: Option<&Path>) -> Option<PathBuf> {
    let home = std::env::var("HOME").ok()?;
    let claude_dir = PathBuf::from(home).join(".claude/projects");

    // Helper to convert a path to Claude's format: /home/user/foo -> -home-user-foo
    let path_to_claude_dir = |path: &Path| -> Option<PathBuf> {
        let path_str = path.to_string_lossy().replace('/', "-");
        // Try with leading dash first (Claude's format)
        let proj_dir = claude_dir.join(format!("-{}", path_str.trim_start_matches('-')));
        if proj_dir.exists() {
            return Some(proj_dir);
        }
        // Try without leading dash
        let proj_dir = claude_dir.join(&path_str);
        if proj_dir.exists() {
            return Some(proj_dir);
        }
        None
    };

    // 1. Explicit project path
    if let Some(proj) = project
        && let Some(dir) = path_to_claude_dir(proj)
    {
        return Some(dir);
    }

    // 2. Git root of current directory
    if let Ok(output) = std::process::Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .output()
        && output.status.success()
        && let Some(dir) =
            path_to_claude_dir(Path::new(String::from_utf8_lossy(&output.stdout).trim()))
    {
        return Some(dir);
    }

    // 3. Current directory
    if let Ok(cwd) = std::env::current_dir()
        && let Some(dir) = path_to_claude_dir(&cwd)
    {
        return Some(dir);
    }

    None
}
