//! Aggregate statistics across sessions.

use super::{
    analyze::{aggregate_sessions, cmd_sessions_analyze_multi},
    session_matches_grep,
};
use crate::output::OutputFormat;
use crate::sessions::{FormatRegistry, LogFormat, SessionFile};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

/// Parse a date string (YYYY-MM-DD) to SystemTime.
pub(crate) fn parse_date(s: &str) -> Option<SystemTime> {
    let parts: Vec<&str> = s.split('-').collect();
    if parts.len() != 3 {
        return None;
    }
    let year: i32 = parts[0].parse().ok()?;
    let month: u32 = parts[1].parse().ok()?;
    let day: u32 = parts[2].parse().ok()?;

    // Convert to days since Unix epoch (rough calculation)
    let days_since_epoch = (year - 1970) as i64 * 365
        + (year - 1970) as i64 / 4 // leap years approx
        + match month {
            1 => 0,
            2 => 31,
            3 => 59,
            4 => 90,
            5 => 120,
            6 => 151,
            7 => 181,
            8 => 212,
            9 => 243,
            10 => 273,
            11 => 304,
            12 => 334,
            _ => return None,
        } as i64
        + day as i64
        - 1;

    let secs = days_since_epoch * 86400;
    if secs < 0 {
        return None;
    }
    Some(SystemTime::UNIX_EPOCH + Duration::from_secs(secs as u64))
}

/// Show aggregate statistics across all sessions.
#[allow(clippy::too_many_arguments)]
pub fn cmd_sessions_stats(
    root: Option<&Path>,
    limit: usize,
    format_name: Option<&str>,
    grep: Option<&str>,
    days: Option<u32>,
    since: Option<&str>,
    until: Option<&str>,
    project_filter: Option<&Path>,
    all_projects: bool,
    group_by: &[String],
    output_format: &OutputFormat,
) -> i32 {
    let json = output_format.is_json();
    let registry = FormatRegistry::new();

    // Get format (default to claude for backwards compatibility)
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

    // Validate group_by values
    let group_project = group_by.iter().any(|g| g == "project");
    let group_day = group_by.iter().any(|g| g == "day");
    for g in group_by {
        if g != "project" && g != "day" {
            eprintln!("Unknown --group-by value: {} (valid: project, day)", g);
            return 1;
        }
    }

    // Compile grep pattern if provided
    let grep_re = grep.and_then(|p| regex::Regex::new(p).ok());
    if grep.is_some() && grep_re.is_none() {
        eprintln!("Invalid grep pattern: {}", grep.unwrap());
        return 1;
    }

    // Get sessions from format
    let mut sessions: Vec<SessionFile> = if all_projects {
        list_all_project_sessions(format)
    } else {
        let project = if let Some(p) = project_filter {
            Some(p)
        } else {
            root
        };
        format.list_sessions(project)
    };

    // Calculate date filters
    let now = SystemTime::now();

    let since_time = if let Some(d) = days {
        Some(now - Duration::from_secs(d as u64 * 86400))
    } else if let Some(s) = since {
        match parse_date(s) {
            Some(t) => Some(t),
            None => {
                eprintln!("Invalid date format: {} (use YYYY-MM-DD)", s);
                return 1;
            }
        }
    } else {
        None
    };

    let until_time = if let Some(u) = until {
        match parse_date(u) {
            Some(t) => Some(t + Duration::from_secs(86400)),
            None => {
                eprintln!("Invalid date format: {} (use YYYY-MM-DD)", u);
                return 1;
            }
        }
    } else {
        None
    };

    // Apply date filters
    if let Some(since) = since_time {
        sessions.retain(|s| s.mtime >= since);
    }
    if let Some(until) = until_time {
        sessions.retain(|s| s.mtime <= until);
    }

    // Apply grep filter if provided
    if let Some(ref re) = grep_re {
        sessions.retain(|s| session_matches_grep(&s.path, re));
    }

    // Sort by time (newest first) and limit
    sessions.sort_by(|a, b| b.mtime.cmp(&a.mtime));
    if limit > 0 {
        sessions.truncate(limit);
    }

    if sessions.is_empty() {
        if json {
            println!("{{}}");
        } else {
            eprintln!("No {} sessions found", format_name.unwrap_or("Claude Code"));
            if days.is_some() || since.is_some() || until.is_some() {
                eprintln!("(with date filter applied)");
            }
        }
        return 0;
    }

    // Show what we're analyzing
    if !json {
        let date_range = if let Some(d) = days {
            format!(" (last {} days)", d)
        } else if since.is_some() || until.is_some() {
            let s = since.unwrap_or("*");
            let u = until.unwrap_or("*");
            format!(" ({} to {})", s, u)
        } else {
            String::new()
        };

        let project_info = if all_projects {
            " across all projects".to_string()
        } else if let Some(p) = project_filter {
            format!(" in {}", p.display())
        } else {
            String::new()
        };

        eprintln!(
            "Analyzing {} sessions{}{}...\n",
            sessions.len(),
            date_range,
            project_info
        );
    }

    // Group and analyze
    if group_project || group_day {
        return cmd_sessions_stats_grouped(
            &sessions,
            group_project,
            group_day,
            format_name,
            output_format,
        );
    }

    // No grouping — analyze all together
    let paths: Vec<_> = sessions.iter().map(|s| s.path.clone()).collect();
    cmd_sessions_analyze_multi(&paths, format_name, output_format)
}

/// Build stats analysis (data only, no printing).
#[allow(clippy::too_many_arguments)]
pub fn build_stats_data(
    root: Option<&Path>,
    limit: usize,
    format_name: Option<&str>,
    grep: Option<&str>,
    days: Option<u32>,
    since: Option<&str>,
    until: Option<&str>,
    project_filter: Option<&Path>,
    all_projects: bool,
) -> Result<crate::sessions::SessionAnalysis, String> {
    let registry = FormatRegistry::new();
    let format: &dyn LogFormat = match format_name {
        Some(name) => registry
            .get(name)
            .ok_or_else(|| format!("Unknown format: {}", name))?,
        None => registry.get("claude").unwrap(),
    };

    let grep_re = grep
        .map(|p| regex::Regex::new(p).map_err(|_| format!("Invalid grep pattern: {}", p)))
        .transpose()?;

    let mut sessions: Vec<SessionFile> = if all_projects {
        list_all_project_sessions(format)
    } else {
        let project = project_filter.or(root);
        format.list_sessions(project)
    };

    let now = SystemTime::now();
    let since_time = if let Some(d) = days {
        Some(now - Duration::from_secs(d as u64 * 86400))
    } else if let Some(s) = since {
        Some(parse_date(s).ok_or_else(|| format!("Invalid date format: {} (use YYYY-MM-DD)", s))?)
    } else {
        None
    };
    let until_time = if let Some(u) = until {
        Some(
            parse_date(u).ok_or_else(|| format!("Invalid date format: {} (use YYYY-MM-DD)", u))?
                + Duration::from_secs(86400),
        )
    } else {
        None
    };

    if let Some(since) = since_time {
        sessions.retain(|s| s.mtime >= since);
    }
    if let Some(until) = until_time {
        sessions.retain(|s| s.mtime <= until);
    }
    if let Some(ref re) = grep_re {
        sessions.retain(|s| session_matches_grep(&s.path, re));
    }

    sessions.sort_by(|a, b| b.mtime.cmp(&a.mtime));
    if limit > 0 {
        sessions.truncate(limit);
    }

    if sessions.is_empty() {
        return Err("No sessions found".to_string());
    }

    let paths: Vec<_> = sessions.iter().map(|s| s.path.clone()).collect();
    aggregate_sessions(&paths, format_name)
        .ok_or_else(|| "No sessions could be analyzed".to_string())
}

/// List sessions from all projects in ~/.claude/projects/
pub(crate) fn list_all_project_sessions(format: &dyn LogFormat) -> Vec<SessionFile> {
    let home = match std::env::var("HOME") {
        Ok(h) => h,
        Err(_) => return Vec::new(),
    };

    let projects_dir = PathBuf::from(home).join(".claude/projects");
    if !projects_dir.exists() {
        return Vec::new();
    }

    let mut all_sessions = Vec::new();

    if let Ok(entries) = std::fs::read_dir(&projects_dir) {
        for entry in entries.filter_map(|e| e.ok()) {
            let proj_dir = entry.path();
            if !proj_dir.is_dir() {
                continue;
            }

            if let Ok(files) = std::fs::read_dir(&proj_dir) {
                for file in files.filter_map(|f| f.ok()) {
                    let path = file.path();
                    if path.extension().and_then(|e| e.to_str()) == Some("jsonl")
                        && let Ok(meta) = path.metadata()
                        && let Ok(mtime) = meta.modified()
                        && format.detect(&path) > 0.5
                    {
                        all_sessions.push(SessionFile { path, mtime });
                    }
                }
            }
        }
    }

    all_sessions
}

/// Build a group key for a session based on active grouping dimensions.
fn group_key(session: &SessionFile, by_project: bool, by_day: bool) -> String {
    let mut parts = Vec::new();

    if by_project {
        parts.push(extract_repo_name(&session.path));
    }

    if by_day {
        parts.push(extract_day(&session.mtime));
    }

    parts.join("/")
}

/// Extract repository name from session path.
/// For paths like ~/.claude/projects/-home-me-git-normalize/session.jsonl, returns "normalize"
fn extract_repo_name(path: &Path) -> String {
    let path_str = path.to_string_lossy();

    if let Some(projects_idx) = path_str.find(".claude/projects/") {
        let after_projects = &path_str[projects_idx + ".claude/projects/".len()..];
        if let Some(slash_idx) = after_projects.find('/') {
            let proj_dir = &after_projects[..slash_idx];

            // Clean up: -home-me-git-normalize -> normalize
            if let Some(last_dash) = proj_dir.rfind('-') {
                return proj_dir[last_dash + 1..].to_string();
            }
            return proj_dir.to_string();
        }
    }

    path.parent()
        .and_then(|p| p.file_name())
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string()
}

/// Extract day string (YYYY-MM-DD) from a SystemTime.
fn extract_day(mtime: &SystemTime) -> String {
    let secs = mtime
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    // Convert Unix timestamp to date components
    let days = secs / 86400;
    // Simplified date calculation (accurate enough for display)
    let mut y = 1970i64;
    let mut remaining = days as i64;

    loop {
        let year_days = if y % 4 == 0 && (y % 100 != 0 || y % 400 == 0) {
            366
        } else {
            365
        };
        if remaining < year_days {
            break;
        }
        remaining -= year_days;
        y += 1;
    }

    let leap = y % 4 == 0 && (y % 100 != 0 || y % 400 == 0);
    let month_days = [
        31,
        if leap { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];

    let mut m = 0usize;
    for (i, &md) in month_days.iter().enumerate() {
        if remaining < md as i64 {
            m = i;
            break;
        }
        remaining -= md as i64;
    }

    format!("{:04}-{:02}-{:02}", y, m + 1, remaining + 1)
}

/// Show statistics grouped by one or more dimensions.
fn cmd_sessions_stats_grouped(
    sessions: &[SessionFile],
    by_project: bool,
    by_day: bool,
    format_name: Option<&str>,
    output_format: &OutputFormat,
) -> i32 {
    let json = output_format.is_json();

    // Group sessions
    let mut groups: HashMap<String, Vec<PathBuf>> = HashMap::new();
    for session in sessions {
        let key = group_key(session, by_project, by_day);
        groups.entry(key).or_default().push(session.path.clone());
    }

    // Sort groups: by day descending if day grouping, otherwise alphabetically
    let mut sorted: Vec<_> = groups.into_iter().collect();
    if by_day && !by_project {
        sorted.sort_by(|a, b| b.0.cmp(&a.0));
    } else {
        sorted.sort_by(|a, b| a.0.cmp(&b.0));
    }

    if json {
        // JSON: object keyed by group name, each value is the analysis result
        let mut results: Vec<String> = Vec::new();
        for (key, paths) in &sorted {
            // Capture the analysis output
            let analysis = analyze_paths_to_json(paths, format_name);
            results.push(format!(
                "{}:{}",
                serde_json::to_string(key).unwrap_or_default(),
                analysis
            ));
        }
        println!("{{{}}}", results.join(","));
        return 0;
    }

    // Text output: header per group
    for (key, paths) in sorted {
        println!("=== {} ({} sessions) ===\n", key, paths.len());

        let result = cmd_sessions_analyze_multi(&paths, format_name, output_format);
        if result != 0 {
            eprintln!("Failed to analyze sessions for {}", key);
            return result;
        }

        println!();
    }

    0
}

/// Run analysis on paths and return JSON string.
fn analyze_paths_to_json(paths: &[PathBuf], format_name: Option<&str>) -> String {
    match aggregate_sessions(paths, format_name) {
        Some(analysis) => serde_json::to_string(&analysis).unwrap_or_else(|_| "{}".to_string()),
        None => "{}".to_string(),
    }
}
