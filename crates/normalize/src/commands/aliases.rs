//! Aliases command - list filter aliases used by --exclude/--only.

use serde::Serialize;
use std::path::Path;

use crate::config::NormalizeConfig;
use crate::filter::{AliasStatus, list_aliases};
use crate::output::OutputFormatter;

/// Alias for serialization
#[derive(Debug, Serialize, schemars::JsonSchema)]
struct AliasItem {
    name: String,
    patterns: Vec<String>,
    status: String,
}

/// Aliases report
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct AliasesReport {
    aliases: Vec<AliasItem>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    detected_languages: Vec<String>,
}

impl AliasesReport {
    /// Build an aliases report from config and detected languages.
    pub fn build(config: &NormalizeConfig, languages: &[String]) -> Self {
        let lang_refs: Vec<&str> = languages.iter().map(|s| s.as_str()).collect();
        let aliases_list = list_aliases(&config.aliases, &lang_refs);
        let alias_items: Vec<AliasItem> = aliases_list
            .iter()
            .map(|a| AliasItem {
                name: a.name.clone(),
                patterns: a.patterns.clone(),
                status: match a.status {
                    AliasStatus::Builtin => "builtin",
                    AliasStatus::Custom => "custom",
                    AliasStatus::Disabled => "disabled",
                    AliasStatus::Overridden => "overridden",
                }
                .to_string(),
            })
            .collect();
        Self {
            aliases: alias_items,
            detected_languages: languages.to_vec(),
        }
    }
}

impl OutputFormatter for AliasesReport {
    fn format_text(&self) -> String {
        let mut lines = vec!["Aliases:".to_string()];

        for alias in &self.aliases {
            let status_suffix = match alias.status.as_str() {
                "builtin" => "",
                "custom" => "  (custom)",
                "disabled" => "  (disabled)",
                "overridden" => "  (overridden)",
                _ => "",
            };

            if alias.patterns.is_empty() {
                lines.push(format!("  @{:<12} (disabled){}", alias.name, status_suffix));
            } else {
                // Show first few patterns
                let patterns_str = if alias.patterns.len() > 3 {
                    format!(
                        "{}, ... (+{})",
                        alias.patterns[..3].join(", "),
                        alias.patterns.len() - 3
                    )
                } else {
                    alias.patterns.join(", ")
                };
                lines.push(format!(
                    "  @{:<12} {}{}",
                    alias.name, patterns_str, status_suffix
                ));
            }
        }

        if !self.detected_languages.is_empty() {
            lines.push(String::new());
            lines.push(format!(
                "Detected languages: {}",
                self.detected_languages.join(", ")
            ));
        }

        lines.join("\n")
    }
}

/// Detect programming languages in the project.
pub fn detect_project_languages(root: &Path) -> Vec<String> {
    use std::collections::HashSet;

    let mut languages = HashSet::new();

    // Walk the project directory (limited depth for performance)
    let walker = ignore::WalkBuilder::new(root)
        .max_depth(Some(5))
        .hidden(false) // Include hidden directories
        .git_ignore(true)
        .build();

    for entry in walker.flatten() {
        let path = entry.path();
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            match ext {
                "go" => {
                    languages.insert("go".to_string());
                }
                "py" | "pyi" => {
                    languages.insert("python".to_string());
                }
                "rs" => {
                    languages.insert("rust".to_string());
                }
                "js" | "mjs" | "cjs" => {
                    languages.insert("javascript".to_string());
                }
                "ts" | "mts" | "cts" => {
                    languages.insert("typescript".to_string());
                }
                "java" => {
                    languages.insert("java".to_string());
                }
                "rb" => {
                    languages.insert("ruby".to_string());
                }
                "c" | "h" => {
                    languages.insert("c".to_string());
                }
                "cpp" | "cc" | "cxx" | "hpp" => {
                    languages.insert("cpp".to_string());
                }
                _ => {}
            }
        }
    }

    let mut result: Vec<_> = languages.into_iter().collect();
    result.sort();
    result
}
