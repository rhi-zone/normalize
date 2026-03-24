//! Standalone CLI service for normalize-filter.
//!
//! Exposes filter utilities as a standalone binary:
//! - `matches` — check if a path passes a filter
//! - `aliases` — list available filter aliases

use crate::{AliasConfig, Filter, list_aliases};
use normalize_output::OutputFormatter;
use schemars::JsonSchema;
use serde::Serialize;
use server_less::cli;
use std::path::Path;

// =============================================================================
// Output types
// =============================================================================

/// Result of a path match check.
#[derive(Serialize, JsonSchema)]
pub struct MatchReport {
    /// Path that was checked.
    pub path: String,
    /// Whether the path passed the filter.
    pub matched: bool,
}

impl OutputFormatter for MatchReport {
    fn format_text(&self) -> String {
        if self.matched {
            format!("{}: included", self.path)
        } else {
            format!("{}: excluded", self.path)
        }
    }
}

/// A single alias entry for display.
#[derive(Serialize, JsonSchema)]
pub struct AliasEntry {
    /// The alias name (without `@` prefix).
    pub name: String,
    /// Whether the alias is enabled or disabled.
    pub status: String,
    /// The glob patterns this alias resolves to.
    pub patterns: Vec<String>,
}

/// List of resolved aliases.
#[derive(Serialize, JsonSchema)]
pub struct AliasesReport {
    /// All known filter aliases with their resolved patterns.
    pub aliases: Vec<AliasEntry>,
}

impl OutputFormatter for AliasesReport {
    fn format_text(&self) -> String {
        let mut out = String::new();
        for alias in &self.aliases {
            if alias.patterns.is_empty() {
                out.push_str(&format!("@{} [{}]\n", alias.name, alias.status));
            } else {
                out.push_str(&format!(
                    "@{} [{}]: {}\n",
                    alias.name,
                    alias.status,
                    alias.patterns.join(", ")
                ));
            }
        }
        out
    }
}

// =============================================================================
// Service
// =============================================================================

/// Standalone CLI service for normalize-filter.
pub struct FilterCliService;

impl FilterCliService {
    pub fn new() -> Self {
        Self
    }
}

impl Default for FilterCliService {
    fn default() -> Self {
        Self::new()
    }
}

impl FilterCliService {
    /// Generic display bridge that routes to `OutputFormatter::format_text()`.
    fn display_output<T: OutputFormatter>(&self, value: &T) -> String {
        value.format_text()
    }
}

#[cli(
    name = "normalize-filter",
    version = "0.1.0",
    description = "File filtering with glob patterns and alias resolution"
)]
impl FilterCliService {
    /// Check if a path is included by the given filters
    #[cli(display_with = "display_output")]
    pub fn matches(
        &self,
        #[param(positional, help = "Path to check")] path: String,
        #[param(help = "Exclude files matching pattern or alias")] exclude: Vec<String>,
        #[param(help = "Include only files matching pattern or alias")] only: Vec<String>,
    ) -> Result<MatchReport, String> {
        let config = AliasConfig::default();
        let filter = Filter::new(&exclude, &only, &config, &[])?;
        let matched = filter.matches(Path::new(&path));
        Ok(MatchReport { path, matched })
    }

    /// List available filter aliases
    #[cli(display_with = "display_output")]
    pub fn aliases(&self) -> Result<AliasesReport, String> {
        let config = AliasConfig::default();
        let resolved = list_aliases(&config, &[]);
        let aliases = resolved
            .into_iter()
            .map(|a| AliasEntry {
                name: a.name,
                status: a.status.to_string(),
                patterns: a.patterns,
            })
            .collect();
        Ok(AliasesReport { aliases })
    }
}
