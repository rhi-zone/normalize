//! Standalone CLI service for normalize-filter.
//!
//! Exposes filter utilities as a standalone binary:
//! - `matches` — check if a path passes a filter
//! - `aliases` — list available filter aliases

use crate::{AliasConfig, Filter, list_aliases};
use schemars::JsonSchema;
use serde::Serialize;
use server_less::cli;
use std::path::Path;

// =============================================================================
// Output types
// =============================================================================

/// Result of a path match check.
#[derive(Serialize, JsonSchema)]
pub struct MatchResult {
    pub path: String,
    pub matched: bool,
}

impl std::fmt::Display for MatchResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.matched {
            write!(f, "{}: included", self.path)
        } else {
            write!(f, "{}: excluded", self.path)
        }
    }
}

/// A single alias entry for display.
#[derive(Serialize, JsonSchema)]
pub struct AliasEntry {
    pub name: String,
    pub status: String,
    pub patterns: Vec<String>,
}

/// List of resolved aliases.
#[derive(Serialize, JsonSchema)]
pub struct AliasesResult {
    pub aliases: Vec<AliasEntry>,
}

impl std::fmt::Display for AliasesResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for alias in &self.aliases {
            if alias.patterns.is_empty() {
                writeln!(f, "@{} [{}]", alias.name, alias.status)?;
            } else {
                writeln!(
                    f,
                    "@{} [{}]: {}",
                    alias.name,
                    alias.status,
                    alias.patterns.join(", ")
                )?;
            }
        }
        Ok(())
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

#[cli(
    name = "normalize-filter",
    version = "0.1.0",
    description = "File filtering with glob patterns and alias resolution"
)]
impl FilterCliService {
    /// Check if a path is included by the given filters
    pub fn matches(
        &self,
        #[param(positional, help = "Path to check")] path: String,
        #[param(help = "Exclude files matching pattern or alias")] exclude: Vec<String>,
        #[param(help = "Include only files matching pattern or alias")] only: Vec<String>,
    ) -> Result<MatchResult, String> {
        let config = AliasConfig::default();
        let filter = Filter::new(&exclude, &only, &config, &[])?;
        let matched = filter.matches(Path::new(&path));
        Ok(MatchResult { path, matched })
    }

    /// List available filter aliases
    pub fn aliases(&self) -> Result<AliasesResult, String> {
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
        Ok(AliasesResult { aliases })
    }
}
