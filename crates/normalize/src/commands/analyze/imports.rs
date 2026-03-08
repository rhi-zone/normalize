//! Import centrality: rank modules by fan-in (how many files import them).
//!
//! Most-imported modules are load-bearing and essential.
//! Least-imported modules are leaf utilities or potential dead weight.
//! Requires a built facts index (`normalize facts rebuild`).

use crate::output::OutputFormatter;
use serde::Serialize;
use std::collections::HashMap;
use std::path::Path;

/// One module and how many distinct files import it.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct ImportEntry {
    /// Module path (e.g. `crate::output`, `serde`, `std::collections`)
    pub module: String,
    /// Number of distinct files that import from this module
    pub fan_in: usize,
    /// Representative names imported from this module (up to 5)
    pub names: Vec<String>,
}

/// Report returned by `analyze imports`.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct ImportCentralityReport {
    /// Total number of distinct modules imported across the codebase
    pub total_modules: usize,
    /// Total import statements recorded
    pub total_imports: usize,
    /// Whether only internal (crate-local) modules are shown
    pub internal_only: bool,
    /// Entries sorted by fan-in descending
    pub entries: Vec<ImportEntry>,
}

impl OutputFormatter for ImportCentralityReport {
    fn format_text(&self) -> String {
        let mut out = Vec::new();

        let scope = if self.internal_only {
            "internal modules"
        } else {
            "all modules"
        };
        out.push(format!(
            "# Import Centrality ({}) — {} modules, {} imports",
            scope, self.total_modules, self.total_imports
        ));
        out.push(String::new());

        if self.entries.is_empty() {
            out.push("No import data found. Run `normalize facts rebuild` first.".to_string());
            return out.join("\n");
        }

        let max_module_len = self
            .entries
            .iter()
            .map(|e| e.module.len())
            .max()
            .unwrap_or(10)
            .min(60);

        out.push(format!(
            "{:<width$}  {:>6}  {}",
            "Module",
            "Fan-in",
            "Imported names",
            width = max_module_len
        ));
        out.push(format!(
            "{}---------{}",
            "-".repeat(max_module_len),
            "-".repeat(40)
        ));

        for entry in &self.entries {
            let module = if entry.module.len() > max_module_len {
                format!(
                    "...{}",
                    &entry.module[entry.module.len() - (max_module_len - 3)..]
                )
            } else {
                entry.module.clone()
            };
            let names = entry.names.join(", ");
            out.push(format!(
                "{:<width$}  {:>6}  {}",
                module,
                entry.fan_in,
                names,
                width = max_module_len
            ));
        }

        out.join("\n")
    }
}

/// Analyze import centrality using the facts index.
pub async fn analyze_import_centrality(
    root: &Path,
    limit: usize,
    internal_only: bool,
) -> Result<ImportCentralityReport, String> {
    let idx = crate::index::ensure_ready(root).await?;

    let raw = idx
        .all_imports()
        .await
        .map_err(|e| format!("Failed to read imports: {}", e))?;

    let total_imports = raw.len();

    // module → set of importing files, and set of names
    let mut fan_in_map: HashMap<String, std::collections::HashSet<String>> = HashMap::new();
    let mut names_map: HashMap<String, Vec<String>> = HashMap::new();

    for (file, module, name, _line) in &raw {
        if module.is_empty() {
            continue;
        }

        // Skip noise: bare relative imports that aren't useful for centrality
        if module == "super" || module == "super::*" || module == "crate" {
            continue;
        }

        if internal_only && !is_internal_module(module) {
            continue;
        }

        fan_in_map
            .entry(module.clone())
            .or_default()
            .insert(file.clone());

        let names = names_map.entry(module.clone()).or_default();
        if !name.is_empty() && name != "*" && !names.contains(name) {
            names.push(name.clone());
        }
    }

    let total_modules = fan_in_map.len();

    let mut entries: Vec<ImportEntry> = fan_in_map
        .into_iter()
        .map(|(module, files)| {
            let fan_in = files.len();
            let mut names = names_map.remove(&module).unwrap_or_default();
            names.sort();
            names.truncate(5);
            ImportEntry {
                module,
                fan_in,
                names,
            }
        })
        .collect();

    normalize_analyze::ranked::rank_and_truncate(
        &mut entries,
        limit,
        |a, b| b.fan_in.cmp(&a.fan_in).then(a.module.cmp(&b.module)),
        |e| e.fan_in as f64,
    );

    Ok(ImportCentralityReport {
        total_modules,
        total_imports,
        internal_only,
        entries,
    })
}

/// Returns true if the module path looks like an internal (crate-local) import.
/// Internal: `crate::*`, `super::something` (not bare `super`)
fn is_internal_module(module: &str) -> bool {
    module.starts_with("crate::") || module.starts_with("super::")
}
