//! Find stale documentation where covered code has changed

use crate::output::OutputFormatter;
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

/// Stale docs analysis report
#[derive(Debug, Serialize, schemars::JsonSchema)]
struct StaleDocsReport {
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
                    let days_stale = (cover.code_modified - doc.doc_modified) / 86400;
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

/// Find docs with stale code coverage
pub fn cmd_stale_docs(root: &Path, json: bool) -> i32 {
    use regex::Regex;

    // Find markdown files with <!-- covers: ... --> declarations
    let covers_re = Regex::new(r"<!--\s*covers:\s*(.+?)\s*-->").unwrap();

    let md_files: Vec<_> = walkdir::WalkDir::new(root)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path().extension().and_then(|s| s.to_str()) == Some("md")
                && !e
                    .path()
                    .components()
                    .any(|c| c.as_os_str().to_string_lossy().starts_with('.'))
        })
        .map(|e| e.path().to_path_buf())
        .collect();

    if md_files.is_empty() {
        let report = StaleDocsReport {
            stale_docs: Vec::new(),
            files_checked: 0,
            files_with_covers: 0,
        };
        let config = crate::config::NormalizeConfig::load(root);
        let format =
            crate::output::OutputFormat::from_cli(json, false, None, false, false, &config.pretty);
        report.print(&format);
        return 0;
    }

    let mut stale_docs: Vec<StaleDoc> = Vec::new();
    let mut files_with_covers = 0;

    for md_file in &md_files {
        let content = match std::fs::read_to_string(md_file) {
            Ok(c) => c,
            Err(_) => continue,
        };

        // Find all covers declarations
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

        // Get doc modification time
        let doc_modified = std::fs::metadata(md_file)
            .and_then(|m| m.modified())
            .map(|t| t.duration_since(std::time::UNIX_EPOCH).unwrap().as_secs())
            .unwrap_or(0);

        let mut stale_covers: Vec<StaleCover> = Vec::new();

        for cover_pattern in covers {
            // Parse comma-separated patterns
            for pattern in cover_pattern.split(',').map(|s| s.trim()) {
                if pattern.is_empty() {
                    continue;
                }

                // Find matching files using glob
                let matching = find_covered_files(root, pattern);

                if matching.is_empty() {
                    continue;
                }

                // Check if any matching file was modified after the doc
                let code_modified = matching
                    .iter()
                    .filter_map(|f| {
                        std::fs::metadata(root.join(f))
                            .and_then(|m| m.modified())
                            .map(|t| t.duration_since(std::time::UNIX_EPOCH).unwrap().as_secs())
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

    let report = StaleDocsReport {
        stale_docs,
        files_checked: md_files.len(),
        files_with_covers,
    };
    let config = crate::config::NormalizeConfig::load(root);
    let format =
        crate::output::OutputFormat::from_cli(json, false, None, false, false, &config.pretty);
    report.print(&format);

    if report.stale_docs.is_empty() { 0 } else { 1 }
}

/// Find files matching a cover pattern (glob or path prefix)
fn find_covered_files(root: &Path, pattern: &str) -> Vec<String> {
    // Check if it's a glob pattern
    if pattern.contains('*') {
        // Use glob matching
        let full_pattern = root.join(pattern);
        glob::glob(full_pattern.to_str().unwrap_or(""))
            .ok()
            .map(|paths| {
                paths
                    .filter_map(|p| p.ok())
                    .filter(|p| p.is_file())
                    .filter_map(|p| p.strip_prefix(root).ok().map(|r| r.display().to_string()))
                    .collect()
            })
            .unwrap_or_default()
    } else {
        // Treat as exact path or prefix
        let target = root.join(pattern);
        if target.is_file() {
            vec![pattern.to_string()]
        } else if target.is_dir() {
            // Find all files in directory
            walkdir::WalkDir::new(&target)
                .into_iter()
                .filter_map(|e| e.ok())
                .filter(|e| e.path().is_file())
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
