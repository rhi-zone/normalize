//! Grammar management commands.

use crate::output::OutputFormatter;
use serde::Serialize;
use std::path::PathBuf;

/// A single grammar entry with its name and file path.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct GrammarEntry {
    pub name: String,
    pub path: String,
}

/// Grammar list report
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct GrammarListReport {
    grammars: Vec<GrammarEntry>,
}

impl GrammarListReport {
    pub fn new(grammars: Vec<(String, PathBuf)>) -> Self {
        Self {
            grammars: grammars
                .into_iter()
                .map(|(name, path)| GrammarEntry {
                    name,
                    path: path.display().to_string(),
                })
                .collect(),
        }
    }
}

impl OutputFormatter for GrammarListReport {
    fn format_text(&self) -> String {
        if self.grammars.is_empty() {
            let mut lines = vec!["No grammars installed.".to_string(), String::new()];
            lines.push("Install grammars with: normalize grammars install".to_string());
            lines.push(
                "Or set NORMALIZE_GRAMMAR_PATH to a directory containing .so/.dylib files"
                    .to_string(),
            );
            lines.join("\n")
        } else {
            let mut lines = vec![format!("Installed grammars ({}):", self.grammars.len())];
            for entry in &self.grammars {
                lines.push(entry.name.clone());
            }
            lines.join("\n")
        }
    }
}

/// Grammar path item
#[derive(Debug, Serialize, schemars::JsonSchema)]
struct GrammarPath {
    source: String,
    path: String,
    exists: bool,
}

/// Grammar paths report
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct GrammarPathsReport {
    paths: Vec<GrammarPath>,
}

impl OutputFormatter for GrammarPathsReport {
    fn format_text(&self) -> String {
        let mut lines = vec!["Grammar search paths:".to_string()];
        for item in &self.paths {
            let exists = if item.exists { "" } else { " (not found)" };
            lines.push(format!("  [{}] {}{}", item.source, item.path, exists));
        }
        lines.join("\n")
    }
}

/// Build a grammar paths report (shared with the service layer).
pub fn build_paths_report() -> GrammarPathsReport {
    let mut raw_paths = Vec::new();

    // Environment variable
    if let Ok(env_path) = std::env::var("NORMALIZE_GRAMMAR_PATH") {
        for p in env_path.split(':') {
            if !p.is_empty() {
                raw_paths.push(("env", PathBuf::from(p)));
            }
        }
    }

    // User config directory
    if let Some(config) = dirs::config_dir() {
        raw_paths.push(("config", config.join("normalize/grammars")));
    }

    let paths: Vec<GrammarPath> = raw_paths
        .iter()
        .map(|(source, path)| GrammarPath {
            source: source.to_string(),
            path: path.display().to_string(),
            exists: path.exists(),
        })
        .collect();

    GrammarPathsReport { paths }
}
