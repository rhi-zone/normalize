//! Check documentation references for broken links

use normalize_output::OutputFormatter;
use normalize_output::diagnostics::{DiagnosticsReport, Issue, Severity};
use serde::Serialize;
use std::path::Path;

static CODE_REF_RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();

/// A broken reference found in documentation
#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
struct BrokenRef {
    file: String,
    line: usize,
    reference: String,
    context: String,
}

/// Report produced by the broken-ref native rule check.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct CheckRefsReport {
    broken_refs: Vec<BrokenRef>,
    files_checked: usize,
    symbols_indexed: usize,
}

impl OutputFormatter for CheckRefsReport {
    fn format_text(&self) -> String {
        let mut lines = Vec::new();
        lines.push("Documentation Reference Check".to_string());
        lines.push(String::new());
        lines.push(format!("Files checked: {}", self.files_checked));
        lines.push(format!("Symbols indexed: {}", self.symbols_indexed));
        lines.push(String::new());

        if self.broken_refs.is_empty() {
            lines.push("No broken references found.".to_string());
        } else {
            lines.push(format!("Broken references ({}):", self.broken_refs.len()));
            lines.push(String::new());
            for r in &self.broken_refs {
                lines.push(format!("  {}:{}: `{}`", r.file, r.line, r.reference));
                if r.context.len() <= 80 {
                    lines.push(format!("    {}", r.context));
                }
            }
        }

        lines.join("\n")
    }
}

/// Derive the normalize data directory for a project root.
///
/// Resolution order:
/// 1. If `NORMALIZE_INDEX_DIR` is set to an absolute path, use it directly.
/// 2. If `NORMALIZE_INDEX_DIR` is set to a relative path, use `$XDG_DATA_HOME/normalize/<relative>`.
/// 3. Otherwise, use `<root>/.normalize`.
fn normalize_dir(root: &Path) -> std::path::PathBuf {
    if let Ok(index_dir) = std::env::var("NORMALIZE_INDEX_DIR") {
        let path = std::path::PathBuf::from(&index_dir);
        if path.is_absolute() {
            return path;
        }
        let data_home = std::env::var("XDG_DATA_HOME")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|_| {
                dirs::home_dir()
                    .unwrap_or_else(|| std::path::PathBuf::from("."))
                    .join(".local/share")
            });
        return data_home.join("normalize").join(path);
    }
    root.join(".normalize")
}

/// Build a CheckRefsReport without printing (for service layer).
pub async fn build_check_refs_report(root: &Path) -> Result<CheckRefsReport, String> {
    // Open index to get known symbols
    let db_path = normalize_dir(root).join("index.sqlite");
    let idx = normalize_facts::FileIndex::open(&db_path, root)
        .await
        .map_err(|e| format!("Failed to open index: {e}"))?;

    // Get all symbol names from index
    let all_symbols = match idx.all_symbol_names().await {
        Ok(syms) => syms,
        Err(e) => {
            tracing::warn!(
                "normalize-native-rules: failed to query symbol names: {}",
                e
            );
            std::collections::HashSet::new()
        }
    };

    if all_symbols.is_empty() {
        return Err("No symbols indexed. Run: normalize structure rebuild".to_string());
    }

    // Find markdown files
    let md_files: Vec<_> = crate::walk::gitignore_walk(root)
        .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("md"))
        .map(|e| e.path().to_path_buf())
        .collect();

    if md_files.is_empty() {
        return Ok(CheckRefsReport {
            broken_refs: Vec::new(),
            files_checked: 0,
            symbols_indexed: all_symbols.len(),
        });
    }

    // Regex for code references: `identifier` or `Module::method` or `Module.method`
    let code_ref_re = CODE_REF_RE.get_or_init(|| {
        regex::Regex::new(r"`([A-Z][a-zA-Z0-9_]*(?:[:\.][a-zA-Z_][a-zA-Z0-9_]*)*)`").unwrap()
    });

    let mut broken_refs: Vec<BrokenRef> = Vec::new();

    for md_file in &md_files {
        let content = match std::fs::read_to_string(md_file) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let rel_path = md_file
            .strip_prefix(root)
            .unwrap_or(md_file)
            .display()
            .to_string();

        let md_dir = md_file.parent().unwrap_or(root);

        let mut in_code_block = false;
        for (line_num, line) in content.lines().enumerate() {
            if line.trim().starts_with("```") {
                in_code_block = !in_code_block;
                continue;
            }
            if in_code_block {
                continue;
            }

            for cap in code_ref_re.captures_iter(line) {
                let reference = &cap[1];

                if is_common_non_symbol(reference) {
                    continue;
                }

                if looks_like_file_path(reference) {
                    // Check if the file exists relative to the markdown file
                    let file_path = md_dir.join(reference.replace("::", "/"));
                    if !file_path.exists() && !root.join(reference.replace("::", "/")).exists() {
                        broken_refs.push(BrokenRef {
                            file: rel_path.clone(),
                            line: line_num + 1,
                            reference: reference.to_string(),
                            context: line.trim().to_string(),
                        });
                    }
                } else if !all_symbols.contains(reference) {
                    broken_refs.push(BrokenRef {
                        file: rel_path.clone(),
                        line: line_num + 1,
                        reference: reference.to_string(),
                        context: line.trim().to_string(),
                    });
                }
            }
        }
    }

    Ok(CheckRefsReport {
        broken_refs,
        files_checked: md_files.len(),
        symbols_indexed: all_symbols.len(),
    })
}

impl From<CheckRefsReport> for DiagnosticsReport {
    fn from(report: CheckRefsReport) -> Self {
        DiagnosticsReport {
            issues: report
                .broken_refs
                .into_iter()
                .map(|r| Issue {
                    file: r.file,
                    line: Some(r.line),
                    column: None,
                    end_line: None,
                    end_column: None,
                    rule_id: "broken-ref".into(),
                    message: if looks_like_file_path(&r.reference) {
                        format!("broken file link `{}`", r.reference)
                    } else {
                        format!("unknown symbol `{}`", r.reference)
                    },
                    severity: Severity::Warning,
                    source: "check-refs".into(),
                    related: vec![],
                    suggestion: None,
                })
                .collect(),
            files_checked: report.files_checked,
            sources_run: vec!["check-refs".into()],
            tool_errors: vec![],
            daemon_cached: false,
        }
    }
}

/// Check if a reference looks like a file path.
///
/// Heuristic: the part after the last `.` is lowercase-only (a file extension),
/// not a capitalized method name or field access.
fn looks_like_file_path(s: &str) -> bool {
    let Some(dot) = s.rfind('.') else {
        return false;
    };
    // SAFETY: '.' is ASCII (1 byte), so dot + 1 is always a valid char boundary
    let ext = &s[dot + 1..];
    !ext.is_empty() && ext.len() <= 5 && ext.chars().all(|c| c.is_ascii_lowercase())
}

/// Check if a string is a common non-symbol pattern (command, path, etc.)
fn is_common_non_symbol(s: &str) -> bool {
    // Skip common patterns that aren't symbols
    matches!(
        s,
        "TODO"
            | "FIXME"
            | "NOTE"
            | "HACK"
            | "XXX"
            | "BUG"
            | "OK"
            | "Err"
            | "Ok"
            | "None"
            | "Some"
            | "True"
            | "False"
            | "String"
            | "Vec"
            | "Option"
            | "Result"
            | "Box"
            | "Arc"
            | "Rc"
            | "HashMap"
            | "HashSet"
            | "BTreeMap"
            | "BTreeSet"
            | "PathBuf"
            | "Path"
            | "File"
            | "Read"
            | "Write"
            | "Debug"
            | "Clone"
            | "Copy"
            | "Default"
            | "Send"
            | "Sync"
            | "Serialize"
            | "Deserialize"
    ) || s.len() < 2
        || s.chars().all(|c| c.is_uppercase() || c == '_') // ALL_CAPS constants
}
