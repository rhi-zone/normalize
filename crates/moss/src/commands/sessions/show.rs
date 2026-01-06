//! Show/analyze a specific session.

use super::analyze::{cmd_sessions_analyze, cmd_sessions_analyze_multi, cmd_sessions_jq};
use super::resolve_session_paths;
use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;

/// Show/analyze a specific session or sessions matching a pattern.
pub fn cmd_sessions_show(
    session_id: &str,
    project: Option<&Path>,
    jq_filter: Option<&str>,
    format: Option<&str>,
    analyze: bool,
    json: bool,
    pretty: bool,
) -> i32 {
    // Find matching session files
    let paths = resolve_session_paths(session_id, project, format);

    if paths.is_empty() {
        eprintln!("No sessions found matching: {}", session_id);
        return 1;
    }

    // If --analyze with multiple sessions, aggregate
    if analyze && paths.len() > 1 {
        return cmd_sessions_analyze_multi(&paths, format, json, pretty);
    }

    // If --analyze with single session
    if analyze {
        return cmd_sessions_analyze(&paths[0], format, json, pretty);
    }

    // If --jq with multiple sessions, apply to all
    if let Some(filter) = jq_filter {
        let mut exit_code = 0;
        for path in &paths {
            let code = cmd_sessions_jq(path, filter);
            if code != 0 {
                exit_code = code;
            }
        }
        return exit_code;
    }

    // Default: dump the raw JSONL (only first match for non-glob)
    let path = &paths[0];
    let file = match File::open(path) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("Failed to open {}: {}", path.display(), e);
            return 1;
        }
    };

    let reader = BufReader::new(file);
    let stdout = std::io::stdout();
    let mut stdout = stdout.lock();

    for line in reader.lines() {
        match line {
            Ok(l) => {
                let _ = writeln!(stdout, "{}", l);
            }
            Err(e) => {
                eprintln!("Read error: {}", e);
                return 1;
            }
        }
    }

    0
}
