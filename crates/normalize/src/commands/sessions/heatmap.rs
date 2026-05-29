//! File edit heatmap: which files were read/written most across a session.

use crate::output::OutputFormatter;
use crate::sessions::{
    ContentBlock, FormatRegistry, LogFormat, Role, SessionFile, parse_session,
    parse_session_with_format,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use super::stats::{list_all_project_sessions_by_mode, parse_date};
use super::{SessionMode, list_sessions_by_mode, session_matches_grep};

/// Classification of a file's activity in a session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum FileClass {
    /// More than 5 write operations — likely fragile/iterative.
    Hot,
    /// Reads only, zero writes — potential test gap.
    ReadOnly,
    /// Mixed read and write activity.
    Normal,
}

impl FileClass {
    pub fn as_str(self) -> &'static str {
        match self {
            FileClass::Hot => "hot",
            FileClass::ReadOnly => "read_only",
            FileClass::Normal => "normal",
        }
    }
}

/// Stats for a single file across the session.
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct FileHeatEntry {
    pub path: String,
    pub read_count: usize,
    pub write_count: usize,
    pub total_touches: usize,
    pub class: Option<FileClass>,
}

impl FileHeatEntry {
    fn classify(&mut self) {
        self.class = Some(if self.write_count > 5 {
            FileClass::Hot
        } else if self.write_count == 0 && self.read_count > 0 {
            FileClass::ReadOnly
        } else {
            FileClass::Normal
        });
    }
}

/// Report for `normalize sessions heatmap`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct HeatmapReport {
    pub session_path: PathBuf,
    pub files: Vec<FileHeatEntry>,
    pub total_files: usize,
    pub hot_files: usize,
    pub read_only_files: usize,
}

impl OutputFormatter for HeatmapReport {
    fn format_text(&self) -> String {
        let mut out = String::new();
        if self.files.is_empty() {
            writeln!(out, "No file operations found.").unwrap();
            return out;
        }
        writeln!(
            out,
            "File edit heatmap — {} files ({} hot, {} read-only)",
            self.total_files, self.hot_files, self.read_only_files
        )
        .unwrap();
        writeln!(out).unwrap();
        writeln!(
            out,
            "  {:<6} {:<6} {:<6}  {:<10}  file",
            "reads", "writes", "total", "class"
        )
        .unwrap();
        writeln!(out, "  {}", "-".repeat(60)).unwrap();
        for f in &self.files {
            let class_str = f.class.map(|c| c.as_str()).unwrap_or("-");
            writeln!(
                out,
                "  {:<6} {:<6} {:<6}  {:<10}  {}",
                f.read_count, f.write_count, f.total_touches, class_str, f.path
            )
            .unwrap();
        }
        out
    }

    fn format_pretty(&self) -> String {
        let mut out = String::new();
        if self.files.is_empty() {
            writeln!(out, "\x1b[2mNo file operations found.\x1b[0m").unwrap();
            return out;
        }
        writeln!(
            out,
            "\x1b[1mFile Edit Heatmap\x1b[0m — {} files (\x1b[31m{} hot\x1b[0m, \x1b[33m{} read-only\x1b[0m)",
            self.total_files, self.hot_files, self.read_only_files
        )
        .unwrap();
        writeln!(out).unwrap();
        writeln!(
            out,
            "  \x1b[2m{:<6} {:<6} {:<6}  {:<10}  file\x1b[0m",
            "reads", "writes", "total", "class"
        )
        .unwrap();
        writeln!(out, "  \x1b[2m{}\x1b[0m", "-".repeat(60)).unwrap();
        for f in &self.files {
            let (class_str, color) = match f.class {
                Some(FileClass::Hot) => ("hot", "\x1b[31m"),
                Some(FileClass::ReadOnly) => ("read_only", "\x1b[33m"),
                _ => ("normal", "\x1b[0m"),
            };
            writeln!(
                out,
                "  {}{:<6}\x1b[0m {:<6} {:<6}  {}{:<10}\x1b[0m  {}",
                color, f.read_count, f.write_count, f.total_touches, color, class_str, f.path
            )
            .unwrap();
        }
        out
    }
}

/// Extract a short normalized file path from a tool call's input.
fn extract_file_path(tool_name: &str, input: &serde_json::Value) -> Option<String> {
    match tool_name {
        "Read" | "Edit" | "Write" => input
            .get("file_path")
            .and_then(|v| v.as_str())
            .map(normalize_path),
        _ => None,
    }
}

/// Normalize an absolute path to a short relative form.
fn normalize_path(path: &str) -> String {
    if !path.starts_with('/') {
        return path.to_string();
    }
    let parts: Vec<&str> = path.split('/').collect();
    for (i, part) in parts.iter().enumerate() {
        if matches!(
            *part,
            "src" | "lib" | "crates" | "tests" | "docs" | "packages"
        ) {
            return parts[i..].join("/");
        }
    }
    path.to_string()
}

/// Accumulate file operations from a session into a HashMap.
fn accumulate_file_ops(
    path: &Path,
    format_name: Option<&str>,
    map: &mut HashMap<String, FileHeatEntry>,
) {
    let session = if let Some(fmt) = format_name {
        match parse_session_with_format(path, fmt) {
            Ok(s) => s,
            Err(_) => return,
        }
    } else {
        match parse_session(path) {
            Ok(s) => s,
            Err(_) => return,
        }
    };

    for turn in &session.turns {
        for msg in &turn.messages {
            if msg.role == Role::Assistant {
                for block in &msg.content {
                    if let ContentBlock::ToolUse { name, input, .. } = block
                        && let Some(file_path) = extract_file_path(name, input)
                    {
                        let entry = map
                            .entry(file_path.clone())
                            .or_insert_with(|| FileHeatEntry {
                                path: file_path,
                                ..Default::default()
                            });
                        match name.as_str() {
                            "Read" => entry.read_count += 1,
                            "Edit" | "Write" => entry.write_count += 1,
                            _ => {}
                        }
                        entry.total_touches += 1;
                    }
                }
            }
        }
    }
}

/// Finalize a HashMap of FileHeatEntry into a sorted, classified report.
fn finalize_report(
    session_path: PathBuf,
    mut map: HashMap<String, FileHeatEntry>,
    top: usize,
) -> HeatmapReport {
    let total_files = map.len();
    for entry in map.values_mut() {
        entry.classify();
    }

    let mut files: Vec<FileHeatEntry> = map.into_values().collect();
    files.sort_by(|a, b| {
        b.write_count
            .cmp(&a.write_count)
            .then(b.total_touches.cmp(&a.total_touches))
    });
    if top > 0 {
        files.truncate(top);
    }

    let hot_files = files
        .iter()
        .filter(|f| f.class == Some(FileClass::Hot))
        .count();
    let read_only_files = files
        .iter()
        .filter(|f| f.class == Some(FileClass::ReadOnly))
        .count();

    HeatmapReport {
        session_path,
        files,
        total_files,
        hot_files,
        read_only_files,
    }
}

/// Build a heatmap report for a single session (by ID).
pub fn build_heatmap_report_for_session(
    session_id: &str,
    project: Option<&Path>,
    format_name: Option<&str>,
    exact: bool,
    top: usize,
) -> Result<HeatmapReport, String> {
    use super::{resolve_session_paths, resolve_session_paths_literal};

    let paths = if exact {
        resolve_session_paths_literal(session_id, project, format_name)
    } else {
        resolve_session_paths(session_id, project, format_name)
    };

    if paths.is_empty() {
        return Err(format!("No sessions found matching: {}", session_id));
    }

    let mut map: HashMap<String, FileHeatEntry> = HashMap::new();
    for path in &paths {
        accumulate_file_ops(path, format_name, &mut map);
    }

    Ok(finalize_report(paths[0].clone(), map, top))
}

/// Build a heatmap report across multiple filtered sessions.
#[allow(clippy::too_many_arguments)]
pub fn build_heatmap_report(
    root: Option<&Path>,
    limit: usize,
    format_name: Option<&str>,
    grep: Option<&str>,
    days: Option<u32>,
    since: Option<&str>,
    until: Option<&str>,
    project_filter: Option<&Path>,
    all_projects: bool,
    mode: &SessionMode,
    agent_type: Option<&str>,
    top: usize,
) -> Result<HeatmapReport, String> {
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
        list_sessions_by_mode(format, project, mode)
    };

    let now = std::time::SystemTime::now();
    let since_time = if let Some(d) = days {
        Some(now - std::time::Duration::from_secs(d as u64 * 86400))
    } else if let Some(s) = since {
        Some(parse_date(s).ok_or_else(|| format!("Invalid date format: {} (use YYYY-MM-DD)", s))?)
    } else {
        None
    };
    let until_time = if let Some(u) = until {
        Some(
            parse_date(u).ok_or_else(|| format!("Invalid date format: {} (use YYYY-MM-DD)", u))?
                + std::time::Duration::from_secs(86400),
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
    if let Some(at) = agent_type {
        let at_lower = at.to_lowercase();
        sessions.retain(|s| {
            s.subagent_type
                .as_deref()
                .is_some_and(|t| t.to_lowercase() == at_lower)
        });
    }

    sessions.sort_by_key(|b| std::cmp::Reverse(b.mtime));
    if limit > 0 {
        sessions.truncate(limit);
    }

    if sessions.is_empty() {
        return Err("No sessions found".to_string());
    }

    let mut map: HashMap<String, FileHeatEntry> = HashMap::new();
    for sf in &sessions {
        accumulate_file_ops(&sf.path, format_name, &mut map);
    }

    Ok(finalize_report(PathBuf::from("."), map, top))
}
