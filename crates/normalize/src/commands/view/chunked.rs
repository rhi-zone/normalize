//! Chunked file viewing for large-file navigation.
//!
//! Implements `--chunk N` (fixed-size chunk navigation) and `--around <pattern>`
//! (context window around a regex/substring match) for the view command.
//! Returns a `ChunkedViewReport` with chunk metadata and the extracted content.

use crate::output::OutputFormatter;
use regex::Regex;
use serde::Serialize;

/// Report for chunked or pattern-anchored file viewing.
///
/// Used by `normalize view chunk <file>` when `--chunk` or `--around` is specified.
/// Fields omitted from JSON when not applicable.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct ChunkedViewReport {
    /// The resolved file path.
    pub file: String,
    /// Chunk number shown (1-indexed), if `--chunk` was used.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chunk: Option<usize>,
    /// Total number of chunks in the file, if `--chunk` was used.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_chunks: Option<usize>,
    /// The pattern searched for, if `--around` was used.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub around: Option<String>,
    /// 1-indexed line number of the first line in the shown range.
    pub line_start: usize,
    /// 1-indexed line number of the last line in the shown range.
    pub line_end: usize,
    /// The extracted file content for the range.
    pub content: String,
    /// 1-indexed line number of the match, if `--around` was used and a match was found.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub match_line: Option<usize>,
    /// Total number of pattern matches in the file (present when `--around` finds matches).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_matches: Option<usize>,
    /// Which match is being shown (1-indexed), if `--around` with `--match-index`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub match_index: Option<usize>,
}

impl OutputFormatter for ChunkedViewReport {
    fn format_text(&self) -> String {
        render_chunked(self, false)
    }

    fn format_pretty(&self) -> String {
        render_chunked(self, true)
    }
}

impl std::fmt::Display for ChunkedViewReport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.format_text())
    }
}

fn render_chunked(report: &ChunkedViewReport, _use_colors: bool) -> String {
    let mut out = String::new();

    // Header line
    if let (Some(chunk), Some(total)) = (report.chunk, report.total_chunks) {
        out.push_str(&format!(
            "# {} — chunk {}/{} (lines {}-{})\n\n",
            report.file, chunk, total, report.line_start, report.line_end
        ));
    } else if let Some(ref pattern) = report.around {
        if let Some(match_line) = report.match_line {
            if let (Some(idx), Some(total)) = (report.match_index, report.total_matches) {
                if total > 1 {
                    out.push_str(&format!(
                        "# {} — match {}/{} for {:?} at line {} (lines {}-{})\n\n",
                        report.file,
                        idx,
                        total,
                        pattern,
                        match_line,
                        report.line_start,
                        report.line_end
                    ));
                } else {
                    out.push_str(&format!(
                        "# {} — {:?} at line {} (lines {}-{})\n\n",
                        report.file, pattern, match_line, report.line_start, report.line_end
                    ));
                }
            } else {
                out.push_str(&format!(
                    "# {} — {:?} at line {} (lines {}-{})\n\n",
                    report.file, pattern, match_line, report.line_start, report.line_end
                ));
            }
        } else {
            out.push_str(&format!(
                "# {} — pattern {:?} not found\n\n",
                report.file, pattern
            ));
        }
    } else {
        out.push_str(&format!(
            "# {} (lines {}-{})\n\n",
            report.file, report.line_start, report.line_end
        ));
    }

    // Navigation hint for multiple matches
    if let (Some(_pattern), Some(total)) = (&report.around, report.total_matches)
        && total > 1
        && let Some(idx) = report.match_index
    {
        out.push_str(&format!(
            "# {} of {} matches shown — use --match-index to navigate\n\n",
            idx, total
        ));
    }

    out.push_str(&report.content);
    if !out.ends_with('\n') {
        out.push('\n');
    }
    out
}

/// Build a `ChunkedViewReport` for `--chunk N` mode.
///
/// `chunk_n` is 1-indexed. `chunk_size` defaults to 100.
pub fn build_chunk_view(
    file_path: &str,
    root: &std::path::Path,
    chunk_n: usize,
    chunk_size: usize,
) -> Result<ChunkedViewReport, String> {
    let (resolved_path, content) = read_file(file_path, root)?;

    let lines: Vec<&str> = content.lines().collect();
    let total_lines = lines.len();

    if total_lines == 0 {
        return Ok(ChunkedViewReport {
            file: resolved_path,
            chunk: Some(1),
            total_chunks: Some(1),
            around: None,
            line_start: 1,
            line_end: 0,
            content: String::new(),
            match_line: None,
            total_matches: None,
            match_index: None,
        });
    }

    let total_chunks = total_lines.div_ceil(chunk_size);

    if chunk_n == 0 || chunk_n > total_chunks {
        return Err(format!(
            "Chunk {} out of range — file has {} chunks of ~{} lines (total {} lines)",
            chunk_n, total_chunks, chunk_size, total_lines
        ));
    }

    let line_start = (chunk_n - 1) * chunk_size + 1;
    let line_end = (chunk_n * chunk_size).min(total_lines);
    let chunk_content = lines[(line_start - 1)..line_end].join("\n");

    Ok(ChunkedViewReport {
        file: resolved_path,
        chunk: Some(chunk_n),
        total_chunks: Some(total_chunks),
        around: None,
        line_start,
        line_end,
        content: chunk_content,
        match_line: None,
        total_matches: None,
        match_index: None,
    })
}

/// Build a `ChunkedViewReport` for `--around <pattern>` mode.
///
/// `context_lines` is the number of lines to show before and after the match (default 50).
/// `match_index` is 1-indexed; defaults to 1 (first match).
pub fn build_around_view(
    file_path: &str,
    root: &std::path::Path,
    pattern: &str,
    context_lines: usize,
    match_index: usize,
) -> Result<ChunkedViewReport, String> {
    let (resolved_path, content) = read_file(file_path, root)?;

    let lines: Vec<&str> = content.lines().collect();
    let total_lines = lines.len();

    // Collect all 1-indexed match line numbers.
    let match_lines = find_match_lines(&lines, pattern)?;
    let total_matches = match_lines.len();

    if total_matches == 0 {
        return Ok(ChunkedViewReport {
            file: resolved_path,
            chunk: None,
            total_chunks: None,
            around: Some(pattern.to_string()),
            line_start: 1,
            line_end: total_lines.max(1),
            content: String::new(),
            match_line: None,
            total_matches: Some(0),
            match_index: None,
        });
    }

    let actual_index = match_index.max(1).min(total_matches);
    let matched_line = match_lines[actual_index - 1]; // 1-indexed line number

    let line_start = matched_line.saturating_sub(context_lines).max(1);
    let line_end = (matched_line + context_lines).min(total_lines);

    let around_content = lines[(line_start - 1)..line_end].join("\n");

    Ok(ChunkedViewReport {
        file: resolved_path,
        chunk: None,
        total_chunks: None,
        around: Some(pattern.to_string()),
        line_start,
        line_end,
        content: around_content,
        match_line: Some(matched_line),
        total_matches: Some(total_matches),
        match_index: Some(actual_index),
    })
}

/// Find all 1-indexed line numbers that match the pattern (regex or substring fallback).
fn find_match_lines(lines: &[&str], pattern: &str) -> Result<Vec<usize>, String> {
    // Try to compile as a regex; fall back to substring search if it fails.
    let re = Regex::new(pattern).ok();

    let mut matches = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        let is_match = if let Some(ref re) = re {
            re.is_match(line)
        } else {
            line.contains(pattern)
        };
        if is_match {
            matches.push(i + 1); // 1-indexed
        }
    }
    Ok(matches)
}

/// Resolve and read a file, returning (display_path, content).
fn read_file(file_path: &str, root: &std::path::Path) -> Result<(String, String), String> {
    let matches = crate::path_resolve::resolve_unified_all(file_path, root);
    let resolved = match matches.len() {
        0 => return Err(format!("File not found: {}", file_path)),
        1 => matches.into_iter().next().unwrap(),
        _ => {
            return Err(format!(
                "Multiple matches for '{}' — be more specific",
                file_path
            ));
        }
    };

    if resolved.is_directory {
        return Err(format!(
            "Cannot use --chunk or --around with a directory: {}",
            file_path
        ));
    }

    let full_path = root.join(&resolved.file_path);
    let content = std::fs::read_to_string(&full_path)
        .map_err(|e| format!("Failed to read file '{}': {}", file_path, e))?;

    Ok((resolved.file_path, content))
}
