//! `stale-doc` native rule — detects documentation files that are likely stale because
//! strongly co-changed code files have been updated more recently.
//!
//! Uses the `co_change_edges` table in the normalize index to find which code files
//! historically change together with each doc file, then compares last-commit timestamps.

use normalize_output::diagnostics::{DiagnosticsReport, Issue, RelatedLocation, Severity};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Configurable options for the `stale-doc` rule.
/// Deserialized from `extra` fields on the `RuleOverride` via `rule_config()`.
#[derive(serde::Deserialize, Default)]
pub struct StaleDocConfig {
    /// Minimum co-change count to consider a pair coupled (default: 3).
    #[serde(default)]
    pub min_co_changes: Option<u64>,
    /// Only flag if the code file was committed more recently by at least N days (default: 0).
    #[serde(default)]
    pub min_lag_days: Option<u64>,
    /// Glob patterns for doc files to check (default: built-in list).
    #[serde(default)]
    pub doc_patterns: Vec<String>,
}

/// Default glob patterns for doc files.
const DEFAULT_DOC_PATTERNS: &[&str] = &["**/*.md", "**/*.rst", "docs/**/*"];

/// Patterns that are explicitly excluded (handled by other rules).
const EXCLUDED_FILENAMES: &[&str] = &["SUMMARY.md"];

/// Returns true if the given relative path matches the doc patterns and is not excluded.
fn is_doc_file(rel_path: &str, patterns: &[glob::Pattern]) -> bool {
    let file_name = std::path::Path::new(rel_path)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    if EXCLUDED_FILENAMES.contains(&file_name.as_str()) {
        return false;
    }
    patterns.iter().any(|p| p.matches(rel_path))
}

/// Open the gix repository at or containing `root`.
fn gix_open(root: &Path) -> Option<gix::Repository> {
    gix::discover(root).ok()
}

/// Returns the Unix timestamp (seconds) of the most recent commit that touches `rel_path`.
///
/// Walks commits newest-first, diffs each against its parent, and returns the committer
/// timestamp of the first commit that includes `rel_path` in the changeset.
pub fn git_last_commit_time(root: &Path, rel_path: &str) -> Option<i64> {
    let repo = gix_open(root)?;
    let head_id = repo.head_id().ok()?;
    let walk = head_id
        .ancestors()
        .sorting(gix::revision::walk::Sorting::ByCommitTime(
            gix::traverse::commit::simple::CommitTimeOrder::NewestFirst,
        ))
        .all()
        .ok()?;

    for info in walk {
        let Ok(info) = info else { continue };
        let Ok(commit) = info.object() else { continue };
        let Ok(tree) = commit.tree() else { continue };
        let parent_tree = info
            .parent_ids()
            .next()
            .and_then(|pid| pid.object().ok())
            .and_then(|obj| obj.into_commit().tree().ok());
        let changes = match repo.diff_tree_to_tree(parent_tree.as_ref(), Some(&tree), None) {
            Ok(c) => c,
            Err(_) => continue,
        };
        let touches = changes.iter().any(|change| {
            use gix::object::tree::diff::ChangeDetached;
            let loc = match change {
                ChangeDetached::Addition { location, .. }
                | ChangeDetached::Deletion { location, .. }
                | ChangeDetached::Modification { location, .. } => location.as_slice(),
                ChangeDetached::Rewrite {
                    source_location, ..
                } => source_location.as_slice(),
            };
            loc == rel_path.as_bytes()
        });
        if touches {
            return info.commit_time;
        }
    }
    None
}

/// Build a `DiagnosticsReport` for the `stale-doc` rule.
///
/// For each documentation file matching the configured patterns, finds co-change partners
/// (code files it historically changes with) and flags the doc file if any partner was
/// committed more recently by at least `min_lag_days`.
///
/// Gracefully degrades if:
/// - The index is not built → returns empty report with a tool error note.
/// - The `co_change_edges` table is empty → returns empty report with a tool error note.
/// - A file's last commit time cannot be determined → skips the comparison.
pub fn build_stale_doc_report(
    root: &Path,
    config: StaleDocConfig,
    files: Option<&[PathBuf]>,
) -> DiagnosticsReport {
    let min_co_changes = config.min_co_changes.unwrap_or(3) as usize;
    let min_lag_secs = config.min_lag_days.unwrap_or(0) * 86400;

    // Build glob patterns for doc file detection.
    let raw_patterns: Vec<&str> = if config.doc_patterns.is_empty() {
        DEFAULT_DOC_PATTERNS.to_vec()
    } else {
        config.doc_patterns.iter().map(String::as_str).collect()
    };
    let patterns: Vec<glob::Pattern> = raw_patterns
        .iter()
        .filter_map(|p| glob::Pattern::new(p).ok())
        .collect();

    // Open the index to get co-change edges.
    let index_path = root.join(".normalize").join("index.sqlite");
    if !index_path.exists() {
        return DiagnosticsReport {
            issues: vec![],
            files_checked: 0,
            sources_run: vec!["stale-doc".into()],
            tool_errors: vec![normalize_output::diagnostics::ToolFailure {
                tool: "stale-doc".into(),
                message: "index not built — run `normalize structure rebuild` to enable stale-doc"
                    .into(),
            }],
            daemon_cached: false,
        };
    }

    // Query co-change edges synchronously by blocking on the async API.
    let edges_result = {
        let db_path = index_path.clone();
        let root_path = root.to_path_buf();
        std::thread::Builder::new()
            .stack_size(8 * 1024 * 1024)
            .spawn(move || {
                let rt = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .ok()?;
                rt.block_on(async {
                    let index = normalize_facts::FileIndex::open(&db_path, &root_path)
                        .await
                        .ok()?;
                    index.query_co_change_edges(min_co_changes).await.ok()?
                })
            })
            .ok()
            .and_then(|h| h.join().ok())
            .flatten()
    };

    let Some(edges) = edges_result else {
        return DiagnosticsReport {
            issues: vec![],
            files_checked: 0,
            sources_run: vec!["stale-doc".into()],
            tool_errors: vec![normalize_output::diagnostics::ToolFailure {
                tool: "stale-doc".into(),
                message: "co_change_edges table is empty or index could not be read — run `normalize structure rebuild`".into(),
            }],
            daemon_cached: false,
        };
    };

    if edges.is_empty() {
        return DiagnosticsReport {
            issues: vec![],
            files_checked: 0,
            sources_run: vec!["stale-doc".into()],
            tool_errors: vec![normalize_output::diagnostics::ToolFailure {
                tool: "stale-doc".into(),
                message: "co_change_edges table is empty — run `normalize structure rebuild` to populate it".into(),
            }],
            daemon_cached: false,
        };
    }

    // Build a map: doc_file -> list of (code_file, co_change_count)
    // Each edge (file_a, file_b, count) can relate a doc file on either side.
    let mut doc_to_partners: HashMap<String, Vec<(String, usize)>> = HashMap::new();
    for (file_a, file_b, count) in &edges {
        let a_is_doc = is_doc_file(file_a, &patterns);
        let b_is_doc = is_doc_file(file_b, &patterns);
        // Only record pairs where exactly one file is a doc (doc ↔ code coupling).
        // Doc-to-doc coupling is not a signal for staleness.
        if a_is_doc && !b_is_doc {
            doc_to_partners
                .entry(file_a.clone())
                .or_default()
                .push((file_b.clone(), *count));
        } else if b_is_doc && !a_is_doc {
            doc_to_partners
                .entry(file_b.clone())
                .or_default()
                .push((file_a.clone(), *count));
        }
    }

    if doc_to_partners.is_empty() {
        return DiagnosticsReport {
            issues: vec![],
            files_checked: 0,
            sources_run: vec!["stale-doc".into()],
            tool_errors: vec![],
            daemon_cached: false,
        };
    }

    // Filter to requested files when --files was provided.
    let doc_files: Vec<String> = if let Some(explicit_files) = files {
        let explicit_rel: Vec<String> = explicit_files
            .iter()
            .filter_map(|p| {
                p.strip_prefix(root)
                    .ok()
                    .map(|r| r.to_string_lossy().into_owned())
            })
            .collect();
        doc_to_partners
            .keys()
            .filter(|k| explicit_rel.contains(k))
            .cloned()
            .collect()
    } else {
        doc_to_partners.keys().cloned().collect()
    };

    let files_checked = doc_files.len();

    // Cache commit times to avoid redundant git walks.
    let mut commit_time_cache: HashMap<String, Option<i64>> = HashMap::new();

    let mut issues = Vec::new();

    for doc_path in &doc_files {
        // Skip doc files that don't exist on disk (deleted, renamed).
        if !root.join(doc_path).exists() {
            continue;
        }

        let doc_time = *commit_time_cache
            .entry(doc_path.clone())
            .or_insert_with(|| git_last_commit_time(root, doc_path));

        let Some(doc_ts) = doc_time else {
            // Can't determine doc's last commit time — skip.
            continue;
        };

        let partners = &doc_to_partners[doc_path];

        // Find the partner most recently committed after the doc.
        let mut worst_partner: Option<(&str, usize, i64)> = None; // (file, count, ts)

        for (partner_path, co_count) in partners {
            // Skip partners that don't exist on disk.
            if !root.join(partner_path).exists() {
                continue;
            }

            let partner_time = *commit_time_cache
                .entry(partner_path.clone())
                .or_insert_with(|| git_last_commit_time(root, partner_path));

            let Some(partner_ts) = partner_time else {
                continue;
            };

            if partner_ts <= doc_ts {
                continue;
            }

            let lag = (partner_ts - doc_ts) as u64;
            if lag < min_lag_secs {
                continue;
            }

            // Pick the partner with the largest lag (most behind).
            let is_worse = worst_partner
                .map(|(_, _, worst_ts)| partner_ts > worst_ts)
                .unwrap_or(true);
            if is_worse {
                worst_partner = Some((partner_path.as_str(), *co_count, partner_ts));
            }
        }

        if let Some((partner_path, co_count, partner_ts)) = worst_partner {
            let lag_days = ((partner_ts - doc_ts) as u64) / 86400;
            issues.push(Issue {
                file: doc_path.clone(),
                line: None,
                column: None,
                end_line: None,
                end_column: None,
                rule_id: "stale-doc".into(),
                message: format!(
                    "possibly stale — {partner_path} was updated {lag_days} day{} more recently (last co-changed {co_count} times)",
                    if lag_days == 1 { "" } else { "s" }
                ),
                severity: Severity::Warning,
                source: "stale-doc".into(),
                related: vec![RelatedLocation {
                    file: partner_path.to_string(),
                    line: None,
                    message: Some(format!("co-changed {co_count} times, updated {lag_days} day{} more recently than doc", if lag_days == 1 { "" } else { "s" })),
                }],
                suggestion: Some(format!(
                    "review {doc_path} to ensure it reflects recent changes in {partner_path}"
                )),
            });
        }
    }

    // Sort by file path for deterministic output.
    issues.sort_by(|a, b| a.file.cmp(&b.file));

    DiagnosticsReport {
        issues,
        files_checked,
        sources_run: vec!["stale-doc".into()],
        tool_errors: vec![],
        daemon_cached: false,
    }
}
