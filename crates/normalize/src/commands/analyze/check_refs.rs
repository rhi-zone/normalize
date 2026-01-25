//! Check documentation references for broken links

use crate::index;
use crate::output::OutputFormatter;
use serde::Serialize;
use std::path::Path;

/// A broken reference found in documentation
#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
struct BrokenRef {
    file: String,
    line: usize,
    reference: String,
    context: String,
}

/// Documentation reference check report
#[derive(Debug, Serialize, schemars::JsonSchema)]
struct CheckRefsReport {
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

/// Check documentation references for broken links
pub fn cmd_check_refs(root: &Path, json: bool) -> i32 {
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(cmd_check_refs_async(root, json))
}

async fn cmd_check_refs_async(root: &Path, json: bool) -> i32 {
    use regex::Regex;

    // Open index to get known symbols
    let idx = match index::FileIndex::open_if_enabled(root).await {
        Some(i) => i,
        None => {
            eprintln!("Indexing disabled or failed. Run: moss index rebuild --call-graph");
            return 1;
        }
    };

    // Get all symbol names from index
    let all_symbols = idx.all_symbol_names().await.unwrap_or_default();

    if all_symbols.is_empty() {
        eprintln!("No symbols indexed. Run: moss index rebuild --call-graph");
        return 1;
    }

    // Find markdown files
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
        let report = CheckRefsReport {
            broken_refs: Vec::new(),
            files_checked: 0,
            symbols_indexed: all_symbols.len(),
        };
        let config = crate::config::NormalizeConfig::load(root);
        let format =
            crate::output::OutputFormat::from_cli(json, false, None, false, false, &config.pretty);
        report.print(&format);
        return 0;
    }

    // Regex for code references: `identifier` or `Module::method` or `Module.method`
    let code_ref_re =
        Regex::new(r"`([A-Z][a-zA-Z0-9_]*(?:[:\.][a-zA-Z_][a-zA-Z0-9_]*)*)`").unwrap();

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

        for (line_num, line) in content.lines().enumerate() {
            for cap in code_ref_re.captures_iter(line) {
                let reference = &cap[1];

                // Extract symbol name (last part after :: or .)
                let symbol_name = reference
                    .rsplit(|c| c == ':' || c == '.')
                    .next()
                    .unwrap_or(reference);

                // Skip common non-symbol patterns
                if is_common_non_symbol(symbol_name) {
                    continue;
                }

                // Check if symbol exists
                if !all_symbols.contains(symbol_name) {
                    // Also check the full reference
                    let full_name = reference.replace("::", ".").replace(".", "::");
                    if !all_symbols.contains(&full_name) && !all_symbols.contains(reference) {
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
    }

    let report = CheckRefsReport {
        broken_refs: broken_refs.clone(),
        files_checked: md_files.len(),
        symbols_indexed: all_symbols.len(),
    };
    let config = crate::config::NormalizeConfig::load(root);
    let format =
        crate::output::OutputFormat::from_cli(json, false, None, false, false, &config.pretty);
    report.print(&format);

    if broken_refs.is_empty() { 0 } else { 1 }
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
