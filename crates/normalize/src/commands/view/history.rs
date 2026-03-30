//! Symbol history via gix blame (line-range commit walk).
//!
//! Uses gix blame with a line range to find the commits that introduced lines
//! in the target range. This is analogous to `git log -L` but implemented via
//! the blame algorithm — each unique commit in the blame output for the range
//! is returned in reverse-chronological order.

use super::report::{ViewHistoryCommit, ViewHistoryReport};
use super::symbol::find_symbol_ci;
use std::path::Path;

/// Find symbol by path (parent/child).
fn find_symbol_by_path<'a>(
    symbols: &'a [normalize_languages::Symbol],
    path: &[String],
    case_insensitive: bool,
) -> Option<&'a normalize_languages::Symbol> {
    if path.is_empty() {
        return None;
    }

    if path.len() == 1 {
        return find_symbol_ci(symbols, &path[0], case_insensitive);
    }

    fn names_match(a: &str, b: &str, ci: bool) -> bool {
        if ci {
            a.eq_ignore_ascii_case(b)
        } else {
            a == b
        }
    }

    let mut current_symbols = symbols;
    for (i, name) in path.iter().enumerate() {
        let found = current_symbols
            .iter()
            .find(|s| names_match(&s.name, name, case_insensitive))?;
        if i == path.len() - 1 {
            return Some(found);
        }
        current_symbols = &found.children;
    }
    None
}

/// Build history report for the view service layer.
pub fn build_view_history_report(
    target: &str,
    root: &Path,
    limit: usize,
    case_insensitive: bool,
) -> Result<ViewHistoryReport, String> {
    let Some(resolved) = crate::path_resolve::resolve_unified(target, root) else {
        return Err(format!("Could not resolve path: {}", target));
    };

    let file_path = resolved.file_path;
    let symbol_path = resolved.symbol_path;
    let symbol_name = symbol_path.first().cloned();

    let full_path = root.join(&file_path);
    if !full_path.exists() {
        return Err(format!("File not found: {}", full_path.display()));
    }

    let content = std::fs::read_to_string(&full_path)
        .map_err(|e| format!("Failed to read {}: {}", full_path.display(), e))?;

    let (start_line, end_line) = if let Some(ref sym_name) = symbol_name {
        let extractor = crate::skeleton::SkeletonExtractor::new();
        let result = extractor.extract(&full_path, &content);

        let found = if symbol_path.len() > 1 {
            find_symbol_by_path(&result.symbols, &symbol_path, case_insensitive)
        } else {
            find_symbol_ci(&result.symbols, sym_name, case_insensitive)
        };

        match found {
            Some(sym) => (sym.start_line, sym.end_line),
            None => return Err(format!("Symbol '{}' not found in {}", sym_name, file_path)),
        }
    } else if !symbol_path.is_empty() {
        return Err("Symbol not found".to_string());
    } else {
        let line_count = content.lines().count();
        (1, line_count)
    };

    build_line_history_service(root, &file_path, start_line, end_line, limit)
}

/// Build history for a line range (service layer) via gix blame.
///
/// Blames the file restricted to [start_line, end_line] (1-based inclusive).
/// Each unique commit_id in the blame output is resolved to author/date/message
/// and returned in reverse-chronological order (newest first), up to `limit`.
fn build_line_history_service(
    root: &Path,
    file_path: &str,
    start_line: usize,
    end_line: usize,
    limit: usize,
) -> Result<ViewHistoryReport, String> {
    let repo = gix::discover(root).map_err(|e| format!("Not a git repository: {e}"))?;
    let head_id = repo
        .head_id()
        .map_err(|e| format!("Failed to resolve HEAD: {e}"))?;

    let path_bstr: &gix::bstr::BStr = file_path.as_bytes().into();
    let ranges = gix::blame::BlameRanges::from_one_based_inclusive_range(
        (start_line as u32)..=(end_line as u32),
    )
    .map_err(|e| format!("Invalid line range {start_line}..={end_line}: {e}"))?;

    let outcome = repo
        .blame_file(
            path_bstr,
            head_id.detach(),
            gix::repository::blame_file::Options {
                ranges,
                ..Default::default()
            },
        )
        .map_err(|e| format!("git blame failed for {file_path}: {e}"))?;

    // Collect unique commit ids preserving order (newest first from blame output).
    let mut seen = std::collections::HashSet::new();
    let mut unique_commit_ids: Vec<gix::hash::ObjectId> = Vec::new();
    for entry in &outcome.entries {
        if seen.insert(entry.commit_id) {
            unique_commit_ids.push(entry.commit_id);
        }
        if unique_commit_ids.len() >= limit {
            break;
        }
    }

    // Resolve each commit id to author/date/message.
    let mut commits = Vec::new();
    for commit_id in unique_commit_ids {
        let Ok(obj) = repo.find_object(commit_id) else {
            continue;
        };
        let commit_obj = obj.into_commit();
        let hash = commit_id.to_hex().to_string();
        use gix::bstr::ByteSlice;
        let author = commit_obj
            .author()
            .ok()
            .map(|a| a.name.to_str_lossy().into_owned())
            .unwrap_or_default();
        let timestamp = commit_obj.time().ok().map(|t| t.seconds).unwrap_or(0);
        let date = crate::commands::analyze::git_utils::format_unix_date(timestamp);
        let message = commit_obj
            .message()
            .ok()
            .map(|m| m.summary().to_str_lossy().into_owned())
            .unwrap_or_default();
        commits.push(ViewHistoryCommit {
            hash,
            author,
            date,
            message,
        });
    }

    Ok(ViewHistoryReport {
        file: file_path.to_string(),
        lines: format!("{}-{}", start_line, end_line),
        commits,
    })
}
