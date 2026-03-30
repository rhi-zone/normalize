//! Skeleton diff — structural changelog between commits.
//!
//! Shows what symbols were added, removed, or changed between a base ref and HEAD,
//! rather than line-level diffs.

use super::git_utils;
use crate::output::OutputFormatter;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Status of a file in the diff.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum FileStatus {
    Added,
    Deleted,
    Modified,
}

/// What kind of change occurred to a symbol.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ChangeKind {
    Added,
    Removed,
    SignatureChanged,
    BodyChanged,
}

/// A single symbol-level change.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SymbolChange {
    pub path: String,
    pub name: String,
    /// Symbol kind (function, struct, method, etc.)
    pub kind: String,
    pub change: ChangeKind,
    pub before_signature: Option<String>,
    pub after_signature: Option<String>,
}

/// Per-file summary of structural changes.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct FileChange {
    pub path: String,
    pub status: FileStatus,
    pub symbols_added: usize,
    pub symbols_removed: usize,
    pub symbols_changed: usize,
}

/// Report from skeleton diff analysis.
#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SkeletonDiffReport {
    pub base_ref: String,
    pub files: Vec<FileChange>,
    pub changes: Vec<SymbolChange>,
    pub total_added: usize,
    pub total_removed: usize,
    pub total_changed: usize,
}

/// Key for symbol matching: (name, kind_str).
type SymbolKey = (String, String);

struct SymbolInfo {
    signature: String,
    line_span: usize,
}

/// Extract a flat map of (name, kind) → SymbolInfo from a symbol tree.
/// Recurses into children with dotted names like `ClassName.method_name`.
fn flatten_symbols(
    symbols: &[normalize_languages::Symbol],
    prefix: &str,
) -> HashMap<SymbolKey, SymbolInfo> {
    let mut map = HashMap::new();
    for sym in symbols {
        let full_name = if prefix.is_empty() {
            sym.name.clone()
        } else {
            format!("{}.{}", prefix, sym.name)
        };
        let kind_str = sym.kind.as_str().to_string();
        map.insert(
            (full_name.clone(), kind_str),
            SymbolInfo {
                signature: sym.signature.clone(),
                line_span: sym.end_line.saturating_sub(sym.start_line) + 1,
            },
        );
        // Recurse into children (methods in classes, etc.)
        let child_map = flatten_symbols(&sym.children, &full_name);
        map.extend(child_map);
    }
    map
}

/// Diff two symbol maps, producing a list of changes for a single file.
fn diff_symbols(
    path: &str,
    before: &HashMap<SymbolKey, SymbolInfo>,
    after: &HashMap<SymbolKey, SymbolInfo>,
) -> Vec<SymbolChange> {
    let mut changes = Vec::new();

    // Added: in after but not before
    for ((name, kind), info) in after {
        if !before.contains_key(&(name.clone(), kind.clone())) {
            changes.push(SymbolChange {
                path: path.to_string(),
                name: name.clone(),
                kind: kind.clone(),
                change: ChangeKind::Added,
                before_signature: None,
                after_signature: Some(info.signature.clone()),
            });
        }
    }

    // Removed: in before but not after
    for ((name, kind), info) in before {
        if !after.contains_key(&(name.clone(), kind.clone())) {
            changes.push(SymbolChange {
                path: path.to_string(),
                name: name.clone(),
                kind: kind.clone(),
                change: ChangeKind::Removed,
                before_signature: Some(info.signature.clone()),
                after_signature: None,
            });
        }
    }

    // Changed: in both — compare signature and body size
    for ((name, kind), before_info) in before {
        if let Some(after_info) = after.get(&(name.clone(), kind.clone())) {
            if before_info.signature != after_info.signature {
                changes.push(SymbolChange {
                    path: path.to_string(),
                    name: name.clone(),
                    kind: kind.clone(),
                    change: ChangeKind::SignatureChanged,
                    before_signature: Some(before_info.signature.clone()),
                    after_signature: Some(after_info.signature.clone()),
                });
            } else {
                // Same signature — check if body size changed significantly (>20% and >2 lines)
                let before_span = before_info.line_span as f64;
                let after_span = after_info.line_span as f64;
                let ratio = if before_span > 0.0 {
                    (after_span - before_span).abs() / before_span
                } else {
                    0.0
                };
                let abs_diff =
                    (after_info.line_span as isize - before_info.line_span as isize).unsigned_abs();
                if ratio > 0.2 && abs_diff > 2 {
                    changes.push(SymbolChange {
                        path: path.to_string(),
                        name: name.clone(),
                        kind: kind.clone(),
                        change: ChangeKind::BodyChanged,
                        before_signature: Some(before_info.signature.clone()),
                        after_signature: Some(after_info.signature.clone()),
                    });
                }
            }
        }
    }

    changes.sort_by(|a, b| a.name.cmp(&b.name));
    changes
}

/// Get file content at a specific git ref.
fn git_show(root: &Path, git_ref: &str, file_path: &str) -> Option<String> {
    git_utils::git_show(root, git_ref, file_path)
}

/// Resolve base ref to merge-base with HEAD.
fn resolve_base(root: &Path, base: &str) -> Result<String, String> {
    git_utils::resolve_merge_base(root, base)
}

/// Get list of changed files with their status (A/D/M).
fn get_diff_files_with_status(
    root: &Path,
    base_ref: &str,
) -> Result<Vec<(FileStatus, String)>, String> {
    let raw = git_utils::git_diff_name_status(root, base_ref)?;
    Ok(raw
        .into_iter()
        .map(|(s, p)| {
            let status = match s {
                git_utils::DiffFileStatus::Added => FileStatus::Added,
                git_utils::DiffFileStatus::Deleted => FileStatus::Deleted,
                git_utils::DiffFileStatus::Modified => FileStatus::Modified,
            };
            (status, p)
        })
        .collect())
}

/// Analyze structural differences between a base ref and HEAD.
pub fn analyze_skeleton_diff(
    root: &Path,
    base: &str,
    exclude_patterns: &[String],
    only_patterns: &[String],
) -> Result<SkeletonDiffReport, String> {
    // resolve_base (via gix) will return an error if not a git repository.
    let base_ref = resolve_base(root, base)?;
    let diff_files = get_diff_files_with_status(root, &base_ref)?;

    let exclude_globs: Vec<glob::Pattern> = exclude_patterns
        .iter()
        .filter_map(|p| glob::Pattern::new(p).ok())
        .collect();
    let only_globs: Vec<glob::Pattern> = only_patterns
        .iter()
        .filter_map(|p| glob::Pattern::new(p).ok())
        .collect();

    let extractor = crate::skeleton::SkeletonExtractor::new();

    let mut files = Vec::new();
    let mut all_changes = Vec::new();

    for (status, file_path) in &diff_files {
        let path = Path::new(file_path);

        // Filter by source file support
        if !super::is_source_file(path) {
            continue;
        }

        // Apply exclude patterns
        if exclude_globs.iter().any(|pat| pat.matches(file_path)) {
            continue;
        }

        // Apply only patterns
        if !only_globs.is_empty() && !only_globs.iter().any(|pat| pat.matches(file_path)) {
            continue;
        }

        let before_symbols = if *status != FileStatus::Added {
            git_show(root, &base_ref, file_path)
                .map(|content| {
                    let result = extractor.extract(&PathBuf::from(file_path), &content);
                    flatten_symbols(&result.symbols, "")
                })
                .unwrap_or_default()
        } else {
            HashMap::new()
        };

        let after_symbols = if *status != FileStatus::Deleted {
            git_show(root, "HEAD", file_path)
                .map(|content| {
                    let result = extractor.extract(&PathBuf::from(file_path), &content);
                    flatten_symbols(&result.symbols, "")
                })
                .unwrap_or_default()
        } else {
            HashMap::new()
        };

        let changes = diff_symbols(file_path, &before_symbols, &after_symbols);

        let symbols_added = changes
            .iter()
            .filter(|c| c.change == ChangeKind::Added)
            .count();
        let symbols_removed = changes
            .iter()
            .filter(|c| c.change == ChangeKind::Removed)
            .count();
        let symbols_changed = changes
            .iter()
            .filter(|c| {
                c.change == ChangeKind::SignatureChanged || c.change == ChangeKind::BodyChanged
            })
            .count();

        // Only include files that have symbol-level changes
        if !changes.is_empty() {
            files.push(FileChange {
                path: file_path.clone(),
                status: *status,
                symbols_added,
                symbols_removed,
                symbols_changed,
            });
            all_changes.extend(changes);
        }
    }

    // Sort files by total changes descending
    files.sort_by(|a, b| {
        let total_a = a.symbols_added + a.symbols_removed + a.symbols_changed;
        let total_b = b.symbols_added + b.symbols_removed + b.symbols_changed;
        total_b.cmp(&total_a)
    });

    let total_added = all_changes
        .iter()
        .filter(|c| c.change == ChangeKind::Added)
        .count();
    let total_removed = all_changes
        .iter()
        .filter(|c| c.change == ChangeKind::Removed)
        .count();
    let total_changed = all_changes
        .iter()
        .filter(|c| c.change == ChangeKind::SignatureChanged || c.change == ChangeKind::BodyChanged)
        .count();

    Ok(SkeletonDiffReport {
        base_ref,
        files,
        changes: all_changes,
        total_added,
        total_removed,
        total_changed,
    })
}

fn format_status_char(status: FileStatus) -> &'static str {
    match status {
        FileStatus::Added => "A",
        FileStatus::Deleted => "D",
        FileStatus::Modified => "M",
    }
}

fn format_change_detail(change: &SymbolChange) -> String {
    match change.change {
        ChangeKind::Added => {
            format!(
                "    + {} ({}) {}",
                change.name,
                change.kind,
                change.after_signature.as_deref().unwrap_or("")
            )
        }
        ChangeKind::Removed => {
            format!(
                "    - {} ({}) {}",
                change.name,
                change.kind,
                change.before_signature.as_deref().unwrap_or("")
            )
        }
        ChangeKind::SignatureChanged => {
            format!("    ~ {}: signature changed", change.name)
        }
        ChangeKind::BodyChanged => {
            format!("    ~ {}: body changed", change.name)
        }
    }
}

impl OutputFormatter for SkeletonDiffReport {
    fn format_text(&self) -> String {
        let mut lines = Vec::new();
        lines.push(format!("# Skeleton Diff (vs {})", self.base_ref));
        lines.push(format!(
            "{} files changed: +{} symbols, -{} symbols, ~{} changed",
            self.files.len(),
            self.total_added,
            self.total_removed,
            self.total_changed
        ));
        lines.push(String::new());

        for file in &self.files {
            lines.push(format!(
                "  {} {:<50} +{} -{} ~{}",
                format_status_char(file.status),
                file.path,
                file.symbols_added,
                file.symbols_removed,
                file.symbols_changed
            ));

            for change in self.changes.iter().filter(|c| c.path == file.path) {
                lines.push(format_change_detail(change));
            }
        }

        lines.join("\n")
    }

    fn format_pretty(&self) -> String {
        let mut lines = Vec::new();
        lines.push(format!(
            "\x1b[1m# Skeleton Diff (vs {})\x1b[0m",
            self.base_ref
        ));
        lines.push(format!(
            "{} files changed: \x1b[32m+{} symbols\x1b[0m, \x1b[31m-{} symbols\x1b[0m, \x1b[33m~{} changed\x1b[0m",
            self.files.len(),
            self.total_added,
            self.total_removed,
            self.total_changed
        ));
        lines.push(String::new());

        for file in &self.files {
            let status_colored = match file.status {
                FileStatus::Added => "\x1b[32mA\x1b[0m",
                FileStatus::Deleted => "\x1b[31mD\x1b[0m",
                FileStatus::Modified => "\x1b[33mM\x1b[0m",
            };
            lines.push(format!(
                "  {} \x1b[1m{:<50}\x1b[0m \x1b[32m+{}\x1b[0m \x1b[31m-{}\x1b[0m \x1b[33m~{}\x1b[0m",
                status_colored,
                file.path,
                file.symbols_added,
                file.symbols_removed,
                file.symbols_changed
            ));

            for change in self.changes.iter().filter(|c| c.path == file.path) {
                let (color, marker) = match change.change {
                    ChangeKind::Added => ("\x1b[32m", "+"),
                    ChangeKind::Removed => ("\x1b[31m", "-"),
                    ChangeKind::SignatureChanged | ChangeKind::BodyChanged => ("\x1b[33m", "~"),
                };
                let detail = match change.change {
                    ChangeKind::Added => {
                        format!(
                            "{} ({}) {}",
                            change.name,
                            change.kind,
                            change.after_signature.as_deref().unwrap_or("")
                        )
                    }
                    ChangeKind::Removed => {
                        format!(
                            "{} ({}) {}",
                            change.name,
                            change.kind,
                            change.before_signature.as_deref().unwrap_or("")
                        )
                    }
                    ChangeKind::SignatureChanged => {
                        format!("{}: signature changed", change.name)
                    }
                    ChangeKind::BodyChanged => {
                        format!("{}: body changed", change.name)
                    }
                };
                lines.push(format!("    {}{} {}\x1b[0m", color, marker, detail));
            }
        }

        lines.join("\n")
    }
}
