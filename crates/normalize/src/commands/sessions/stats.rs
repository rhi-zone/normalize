//! Aggregate statistics across sessions.

use super::{
    analyze::{aggregate_sessions, print_sessions_analysis},
    session_matches_grep,
    sort::{DefaultDir, SortDir, SortSpec},
};
use crate::sessions::{FormatRegistry, LogFormat, SessionFile};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::time::{Duration, SystemTime};

/// Fields that `sessions stats` can be sorted on (affects the per-tool rows).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatsSortField {
    /// Sort tool rows by call count (numeric, default desc).
    Calls,
    /// Sort tool rows by error count (numeric, default desc).
    Errors,
    /// Sort tool rows by name (string, default asc).
    Name,
}

impl DefaultDir for StatsSortField {
    fn default_dir(self) -> SortDir {
        match self {
            StatsSortField::Calls => SortDir::Descending,
            StatsSortField::Errors => SortDir::Descending,
            StatsSortField::Name => SortDir::Ascending,
        }
    }
}

impl FromStr for StatsSortField {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "calls" | "call" => Ok(StatsSortField::Calls),
            "errors" | "error" | "err" => Ok(StatsSortField::Errors),
            "name" | "tool" => Ok(StatsSortField::Name),
            _ => Err(format!(
                "unknown sort field '{}': expected 'calls', 'errors', or 'name'",
                s
            )),
        }
    }
}

/// Convert a `SortSpec<StatsSortField>` to the hint string expected by `sort_tool_stats_by_hint`.
fn stats_sort_hint(spec: &SortSpec<StatsSortField>) -> Option<String> {
    let key = spec.keys.first()?;
    let prefix = match key.dir {
        SortDir::Ascending => "+",
        SortDir::Descending => "-",
    };
    let name = match key.field {
        StatsSortField::Calls => "calls",
        StatsSortField::Errors => "errors",
        StatsSortField::Name => "name",
    };
    Some(format!("{}{}", prefix, name))
}

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
pub fn show_stats_grouped(
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
    mode: &super::SessionMode,
    agent_type: Option<&str>,
) -> i32 {
    let json = false;
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
        None => match registry.get("claude") {
            Some(f) => f,
            None => {
                eprintln!("Claude format not available (compile with feature = format-claude)");
                return 1;
            }
        },
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
        list_all_project_sessions_by_mode(format, mode)
    } else {
        let project = if let Some(p) = project_filter {
            Some(p)
        } else {
            root
        };
        super::list_sessions_by_mode(format, project, mode)
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

    // Agent type filtering (case-insensitive match on subagent_type)
    if let Some(at) = agent_type {
        let at_lower = at.to_lowercase();
        sessions.retain(|s| {
            s.subagent_type
                .as_deref()
                .is_some_and(|t| t.to_lowercase() == at_lower)
        });
    }

    // Sort by time (newest first) and limit
    sessions.sort_by(|a, b| b.mtime.cmp(&a.mtime));
    let total_before_limit = sessions.len();
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
        if let Some(t) = super::TruncationInfo::if_truncated(total_before_limit, limit) {
            eprintln!("{}\n", t.notice());
        }
    }

    // Group and analyze
    if group_project || group_day {
        return show_stats_grouped_by_key(&sessions, group_project, group_day, format_name);
    }

    // No grouping — analyze all together
    let paths: Vec<_> = sessions.iter().map(|s| s.path.clone()).collect();
    print_sessions_analysis(&paths, format_name)
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
    mode: &super::SessionMode,
    agent_type: Option<&str>,
    sort: Option<&str>,
) -> Result<crate::sessions::SessionAnalysisReport, String> {
    let registry = FormatRegistry::new();
    let format: &dyn LogFormat = match format_name {
        Some(name) => registry
            .get(name)
            .ok_or_else(|| format!("Unknown format: {}", name))?,
        None => registry.get("claude").ok_or_else(|| {
            "Claude format not available (compile with feature = format-claude)".to_string()
        })?,
    };

    let grep_re = grep
        .map(|p| regex::Regex::new(p).map_err(|_| format!("Invalid grep pattern: {}", p)))
        .transpose()?;

    let mut sessions: Vec<SessionFile> = if all_projects {
        list_all_project_sessions_by_mode(format, mode)
    } else {
        let project = project_filter.or(root);
        super::list_sessions_by_mode(format, project, mode)
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

    // Agent type filtering (case-insensitive match on subagent_type)
    if let Some(at) = agent_type {
        let at_lower = at.to_lowercase();
        sessions.retain(|s| {
            s.subagent_type
                .as_deref()
                .is_some_and(|t| t.to_lowercase() == at_lower)
        });
    }

    sessions.sort_by(|a, b| b.mtime.cmp(&a.mtime));
    let total_before_limit = sessions.len();
    if limit > 0 {
        sessions.truncate(limit);
    }

    if sessions.is_empty() {
        return Err("No sessions found".to_string());
    }

    if let Some(t) = super::TruncationInfo::if_truncated(total_before_limit, limit) {
        eprintln!("{}", t.notice());
    }

    let paths: Vec<_> = sessions.iter().map(|s| s.path.clone()).collect();
    let mut report = aggregate_sessions(&paths, format_name)
        .ok_or_else(|| "No sessions could be analyzed".to_string())?;

    // Apply sort hint to tool rows in formatted output.
    if let Some(s) = sort {
        let sort_spec = SortSpec::<StatsSortField>::parse(s)?;
        report.tool_sort = stats_sort_hint(&sort_spec);
    }

    Ok(report)
}

/// List project directories under ~/.claude/projects/.
pub(crate) fn list_all_project_dirs(format: &dyn LogFormat) -> Vec<PathBuf> {
    let _ = format; // future: could use format to filter
    let home = match std::env::var("HOME") {
        Ok(h) => h,
        Err(_) => return Vec::new(),
    };
    let projects_dir = PathBuf::from(home).join(".claude/projects");
    if !projects_dir.exists() {
        return Vec::new();
    }
    let mut dirs = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&projects_dir) {
        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.is_dir() {
                dirs.push(path);
            }
        }
    }
    dirs
}

/// List all project sessions (interactive + subagent) filtered by mode.
pub(crate) fn list_all_project_sessions_by_mode(
    format: &dyn LogFormat,
    mode: &super::SessionMode,
) -> Vec<SessionFile> {
    use normalize_chat_sessions::{list_jsonl_sessions, list_subagent_sessions};
    let mut all = Vec::new();
    for dir in list_all_project_dirs(format) {
        match mode {
            super::SessionMode::Interactive => {
                let mut sessions = list_jsonl_sessions(&dir);
                sessions.retain(|s| format.detect(&s.path) > 0.5);
                all.extend(sessions);
            }
            super::SessionMode::Subagent => {
                all.extend(list_subagent_sessions(&dir));
            }
            super::SessionMode::All => {
                let mut sessions = list_jsonl_sessions(&dir);
                sessions.retain(|s| format.detect(&s.path) > 0.5);
                all.extend(sessions);
                all.extend(list_subagent_sessions(&dir));
            }
        }
    }
    all
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
fn show_stats_grouped_by_key(
    sessions: &[SessionFile],
    by_project: bool,
    by_day: bool,
    format_name: Option<&str>,
) -> i32 {
    let json = false;

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

        let result = print_sessions_analysis(&paths, format_name);
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
