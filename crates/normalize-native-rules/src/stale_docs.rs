//! Find stale documentation where covered code has changed

use normalize_output::OutputFormatter;
use normalize_output::diagnostics::{DiagnosticsReport, Issue, RelatedLocation, Severity};
use serde::Serialize;
use std::path::Path;

/// A doc file with stale code coverage
#[derive(Debug, Serialize, schemars::JsonSchema)]
struct StaleDoc {
    doc_path: String,
    doc_modified: u64,
    stale_covers: Vec<StaleCover>,
}

/// A stale coverage declaration
#[derive(Debug, Serialize, schemars::JsonSchema)]
struct StaleCover {
    pattern: String,
    code_modified: u64,
    matching_files: Vec<String>,
}

/// Report produced by the stale-doc native rule check.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct StaleDocsReport {
    stale_docs: Vec<StaleDoc>,
    files_checked: usize,
    files_with_covers: usize,
}

impl OutputFormatter for StaleDocsReport {
    fn format_text(&self) -> String {
        let mut lines = Vec::new();
        lines.push("Stale Documentation Check".to_string());
        lines.push(String::new());
        lines.push(format!("Files checked: {}", self.files_checked));
        lines.push(format!("Files with covers: {}", self.files_with_covers));
        lines.push(String::new());

        if self.stale_docs.is_empty() {
            lines.push("No stale docs found. All covered code is older than docs.".to_string());
        } else {
            lines.push(format!("Stale docs ({}):", self.stale_docs.len()));
            lines.push(String::new());
            for doc in &self.stale_docs {
                lines.push(format!("  {}", doc.doc_path));
                for cover in &doc.stale_covers {
                    let days_stale = cover.code_modified.saturating_sub(doc.doc_modified) / 86400;
                    lines.push(format!(
                        "    {} ({} files, ~{} days stale)",
                        cover.pattern,
                        cover.matching_files.len(),
                        days_stale
                    ));
                }
            }
        }

        lines.join("\n")
    }
}

/// Build a StaleDocsReport without printing (for service layer).
pub fn build_stale_docs_report(root: &Path) -> StaleDocsReport {
    use std::sync::OnceLock;

    static COVERS_RE: OnceLock<regex::Regex> = OnceLock::new();
    // normalize-syntax-allow: rust/unwrap-in-impl - compile-time constant regex pattern
    let covers_re =
        COVERS_RE.get_or_init(|| regex::Regex::new(r"<!--\s*covers:\s*(.+?)\s*-->").unwrap());

    let md_files: Vec<_> = crate::walk::gitignore_walk(root)
        .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("md"))
        .map(|e| e.path().to_path_buf())
        .collect();

    if md_files.is_empty() {
        return StaleDocsReport {
            stale_docs: Vec::new(),
            files_checked: 0,
            files_with_covers: 0,
        };
    }

    let mut stale_docs: Vec<StaleDoc> = Vec::new();
    let mut files_with_covers = 0;

    for md_file in &md_files {
        let content = match std::fs::read_to_string(md_file) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let covers: Vec<String> = covers_re
            .captures_iter(&content)
            .map(|cap| cap[1].to_string())
            .collect();

        if covers.is_empty() {
            continue;
        }

        files_with_covers += 1;

        let rel_path = md_file
            .strip_prefix(root)
            .unwrap_or(md_file)
            .display()
            .to_string();

        let doc_modified = std::fs::metadata(md_file)
            .and_then(|m| m.modified())
            .map(|t| {
                t.duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or(std::time::Duration::ZERO)
                    .as_secs()
            })
            .unwrap_or(0);

        let mut stale_covers: Vec<StaleCover> = Vec::new();

        for cover_pattern in covers {
            for pattern in cover_pattern.split(',').map(|s| s.trim()) {
                if pattern.is_empty() {
                    continue;
                }

                let matching = find_covered_files(root, pattern);

                if matching.is_empty() {
                    continue;
                }

                let code_modified = matching
                    .iter()
                    .filter_map(|f| {
                        std::fs::metadata(root.join(f))
                            .and_then(|m| m.modified())
                            .map(|t| {
                                t.duration_since(std::time::UNIX_EPOCH)
                                    .unwrap_or(std::time::Duration::ZERO)
                                    .as_secs()
                            })
                            .ok()
                    })
                    .max()
                    .unwrap_or(0);

                if code_modified > doc_modified {
                    stale_covers.push(StaleCover {
                        pattern: pattern.to_string(),
                        code_modified,
                        matching_files: matching,
                    });
                }
            }
        }

        if !stale_covers.is_empty() {
            stale_docs.push(StaleDoc {
                doc_path: rel_path,
                doc_modified,
                stale_covers,
            });
        }
    }

    StaleDocsReport {
        stale_docs,
        files_checked: md_files.len(),
        files_with_covers,
    }
}

impl From<StaleDocsReport> for DiagnosticsReport {
    fn from(report: StaleDocsReport) -> Self {
        DiagnosticsReport {
            issues: report
                .stale_docs
                .into_iter()
                .flat_map(|doc| {
                    doc.stale_covers.into_iter().map(move |cover| {
                        let days_stale =
                            cover.code_modified.saturating_sub(doc.doc_modified) / 86400;
                        Issue {
                            file: doc.doc_path.clone(),
                            line: None,
                            column: None,
                            end_line: None,
                            end_column: None,
                            rule_id: "stale-doc".into(),
                            message: format!(
                                "covers `{}` ({} files, ~{} days stale)",
                                cover.pattern,
                                cover.matching_files.len(),
                                days_stale
                            ),
                            severity: Severity::Info,
                            source: "stale-docs".into(),
                            related: cover
                                .matching_files
                                .iter()
                                .map(|f| RelatedLocation {
                                    file: f.clone(),
                                    line: None,
                                    message: None,
                                })
                                .collect(),
                            suggestion: Some(format!(
                                "update {} to reflect recent changes",
                                doc.doc_path
                            )),
                        }
                    })
                })
                .collect(),
            files_checked: report.files_checked,
            sources_run: vec!["stale-docs".into()],
            tool_errors: vec![],
            daemon_cached: false,
        }
    }
}

/// Find files matching a cover pattern (glob or path prefix)
fn find_covered_files(root: &Path, pattern: &str) -> Vec<String> {
    // Reject patterns that could escape the project root via path traversal.
    // Check both the pattern string and the resolved full path.
    let pattern_path = std::path::Path::new(pattern);
    if pattern_path
        .components()
        .any(|c| matches!(c, std::path::Component::ParentDir))
    {
        return vec![];
    }

    // Check if it's a glob pattern
    if pattern.contains('*') {
        // Use glob matching
        let full_pattern = root.join(pattern);
        // Guard: ensure the constructed glob path still lives under root
        // (after stripping glob wildcards, the prefix must be under root).
        let non_glob_prefix: std::path::PathBuf = full_pattern
            .components()
            .take_while(|c| !c.as_os_str().to_string_lossy().contains('*'))
            .collect();
        if let (Ok(canon_prefix), Ok(canon_root)) =
            (non_glob_prefix.canonicalize(), root.canonicalize())
            && !canon_prefix.starts_with(&canon_root)
        {
            return vec![];
        }
        match glob::glob(full_pattern.to_str().unwrap_or("")) {
            Err(e) => {
                tracing::warn!(
                    "normalize-native-rules: invalid glob pattern {:?}: {}",
                    full_pattern,
                    e
                );
                vec![]
            }
            Ok(paths) => paths
                .filter_map(|p| p.ok())
                .filter(|p| p.is_file())
                .filter_map(|p| p.strip_prefix(root).ok().map(|r| r.display().to_string()))
                .collect(),
        }
    } else {
        // Treat as exact path or prefix
        let target = root.join(pattern);
        if target.is_file() {
            vec![pattern.to_string()]
        } else if target.is_dir() {
            // Find all files in directory
            crate::walk::gitignore_walk(&target)
                .filter(|e| e.file_type().is_some_and(|ft| ft.is_file()))
                .filter_map(|e| {
                    e.path()
                        .strip_prefix(root)
                        .ok()
                        .map(|r| r.display().to_string())
                })
                .collect()
        } else {
            vec![]
        }
    }
}
