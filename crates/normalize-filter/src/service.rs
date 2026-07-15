//! Standalone CLI service for normalize-filter — the `filter` verb.
//!
//! Exposes filter utilities:
//! - `matches` — check if a path passes a filter
//! - `aliases` — list available filter aliases
//!
//! Both commands load the `[aliases]` slice from the project's
//! `.normalize/config.toml` and detect the project's languages so that
//! language-aware built-ins like `@tests` resolve correctly — without any
//! dependency on the main crate's `NormalizeConfig`.

use crate::{AliasConfig, Filter, list_aliases};
use normalize_output::OutputFormatter;
use schemars::JsonSchema;
use serde::Serialize;
use server_less::cli;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

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
pub struct AliasReportEntry {
    /// The alias name (without `@` prefix).
    pub name: String,
    /// The syntax type (command, glob, sql, path).
    pub syntax: String,
    /// Whether the alias is enabled or disabled.
    pub status: String,
    /// The alias value (patterns for glob, command string for command, etc.).
    pub value: Vec<String>,
    /// Human-readable description.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// List of resolved aliases.
#[derive(Serialize, JsonSchema)]
pub struct AliasesReport {
    /// All known aliases with their definitions.
    pub aliases: Vec<AliasReportEntry>,
    /// Languages detected in the project (used to resolve `@tests` and friends).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub detected_languages: Vec<String>,
}

impl OutputFormatter for AliasesReport {
    fn format_text(&self) -> String {
        let mut out = String::new();
        for alias in &self.aliases {
            out.push_str(&format!(
                "@{} [{}, {}]",
                alias.name, alias.status, alias.syntax
            ));
            if !alias.value.is_empty() {
                if alias.value.len() == 1 {
                    out.push_str(&format!(": {}", alias.value[0]));
                } else {
                    out.push_str(&format!(": {}", alias.value.join(", ")));
                }
            }
            out.push('\n');
            if let Some(desc) = &alias.description {
                out.push_str(&format!("  {}\n", desc));
            }
        }
        if !self.detected_languages.is_empty() {
            out.push('\n');
            out.push_str(&format!(
                "Detected languages: {}\n",
                self.detected_languages.join(", ")
            ));
        }
        out
    }
}

// =============================================================================
// Config-slice loading + language detection (CLI-only, no NormalizeConfig dep)
// =============================================================================

/// Resolve the root directory from an optional CLI argument.
fn resolve_root(root: Option<String>) -> Result<PathBuf, String> {
    root.map(PathBuf::from)
        .map(Ok)
        .unwrap_or_else(std::env::current_dir)
        .map_err(|e| format!("failed to determine root directory: {e}"))
}

/// Load the `[aliases]` section with ancestor-directory walking.
///
/// Walks from `root` up to the git root (or filesystem root), collecting all
/// `.normalize/config.toml` files plus the global config. Inner (closer to
/// `root`) overrides outer. Returns an empty (default) config if no file
/// declares `[aliases]` — the built-in aliases still apply.
fn load_alias_config(root: &Path) -> AliasConfig {
    normalize_config_paths::load_section_hierarchical(root, "aliases")
}

/// Detect programming languages present under `root` (bounded depth walk).
///
/// Mirrors the main crate's detection: maps file paths to language names via the
/// `normalize-languages` registry. Used so `@tests` expands to the right globs.
fn detect_project_languages(root: &Path) -> Vec<String> {
    let mut languages = HashSet::new();
    let walker = ignore::WalkBuilder::new(root)
        .max_depth(Some(5))
        .hidden(false)
        .git_ignore(true)
        .build();
    for entry in walker.flatten() {
        if let Some(lang) = normalize_languages::support_for_path(entry.path()) {
            languages.insert(lang.name().to_string());
        }
    }
    let mut result: Vec<_> = languages.into_iter().collect();
    result.sort();
    result
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
    name = "filter",
    version = "0.3.2",
    description = "Filter files by glob patterns and inspect --exclude/--only aliases"
)]
impl FilterCliService {
    /// Check if a path is included by the given filters
    ///
    /// Resolves `@alias` values against the project's `[aliases]` config and
    /// detected languages, then reports whether the path passes.
    ///
    /// Examples:
    ///   normalize filter matches src/main.rs --only "*.rs"
    ///   normalize filter matches foo_test.go --exclude @tests
    #[cli(display_with = "display_output")]
    pub fn matches(
        &self,
        #[param(positional, help = "Path to check")] path: String,
        #[param(help = "Exclude files matching pattern or alias")] exclude: Vec<String>,
        #[param(help = "Include only files matching pattern or alias")] only: Vec<String>,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
    ) -> Result<MatchReport, String> {
        let root_path = resolve_root(root)?;
        let config = load_alias_config(&root_path);
        let languages = detect_project_languages(&root_path);
        let lang_refs: Vec<&str> = languages.iter().map(String::as_str).collect();
        let filter = Filter::new(&exclude, &only, &config, &lang_refs)?;
        for warning in filter.warnings() {
            eprintln!("warning: {warning}");
        }
        let matched = filter.matches(Path::new(&path));
        Ok(MatchReport { path, matched })
    }

    /// List available filter aliases
    ///
    /// Shows built-in and config-defined `@aliases` usable with `--exclude` /
    /// `--only`, resolved for the project's detected languages.
    ///
    /// Examples:
    ///   normalize filter aliases                # list all filter aliases
    #[cli(display_with = "display_output")]
    pub fn aliases(
        &self,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
    ) -> Result<AliasesReport, String> {
        let root_path = resolve_root(root)?;
        let config = load_alias_config(&root_path);
        let languages = detect_project_languages(&root_path);
        let lang_refs: Vec<&str> = languages.iter().map(String::as_str).collect();
        let resolved = list_aliases(&config, &lang_refs);
        let aliases = resolved
            .into_iter()
            .map(|a| AliasReportEntry {
                name: a.name,
                syntax: a.syntax.to_string(),
                status: a.status.to_string(),
                value: a.value.as_strings(),
                description: a.description,
            })
            .collect();
        Ok(AliasesReport {
            aliases,
            detected_languages: languages,
        })
    }
}
