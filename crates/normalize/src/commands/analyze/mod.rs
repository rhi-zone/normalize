//! Analyze command - run analysis on target.

pub mod activity;
pub mod architecture;
pub mod ast;
pub mod budget;
pub mod call_complexity;
pub mod call_graph;
pub mod ceremony;
pub mod check_examples;
pub mod check_refs;
pub mod clusters;
pub mod complexity;
pub mod contributors;
pub mod coupling;
pub mod coupling_clusters;
pub mod coupling_views;
pub mod coverage;
pub mod cross_repo_health;
pub mod density;
pub mod depth_map;
pub mod docs;
pub mod duplicates;
pub mod duplicates_views;
pub mod files;
pub mod graph;
pub mod hotspots;
pub mod impact;
pub mod imports;
pub mod layering;
pub mod length;
pub mod module_health;
pub mod ownership;
pub mod patterns;
pub mod provenance;
pub mod query;
pub mod repo_coupling;
pub mod report;
pub mod rules_cmd;
pub mod security;
pub mod size;
pub mod skeleton_diff;
pub mod stale_docs;
pub mod summary;
pub mod surface;
pub mod test_gaps;
pub mod test_ratio;
pub mod trace;
pub mod trend;
pub mod uniqueness;

use crate::filter::Filter;
use normalize_core::Merge;
pub use normalize_syntax_rules::{RuleOverride, RulesConfig};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Analyze command configuration.
#[derive(Debug, Clone, Deserialize, Serialize, Default, Merge, schemars::JsonSchema)]
#[serde(default)]
pub struct AnalyzeConfig {
    /// Default complexity threshold for filtering
    pub threshold: Option<usize>,
    /// Use compact output by default (for --overview)
    pub compact: Option<bool>,
    /// Run health analysis by default
    pub health: Option<bool>,
    /// Run complexity analysis by default
    pub complexity: Option<bool>,
    /// Run security analysis by default
    pub security: Option<bool>,
    /// Run duplicate function detection by default
    pub duplicate_functions: Option<bool>,
    /// Weights for final grade calculation
    pub weights: Option<AnalyzeWeights>,
    /// Exclude interface implementations from doc coverage (default: true)
    /// This excludes trait impl methods in Rust, @Override methods in Java, etc.
    pub exclude_interface_impls: Option<bool>,
    /// Patterns to exclude from hotspots analysis (e.g., generated code, lock files)
    #[serde(default)]
    pub hotspots_exclude: Vec<String>,
    /// Syntax rules configuration
    #[serde(default)]
    pub rules: RulesConfig,
    /// Fact rules (Datalog) configuration
    #[serde(default, rename = "facts-rules")]
    pub facts_rules: normalize_facts_rules_interpret::FactsRulesConfig,
    /// Default lines of context to show in query preview
    #[serde(rename = "query-context-lines")]
    pub query_context_lines: Option<usize>,
    /// Patterns to exclude from all analysis (e.g., generated or intentionally parallel code)
    #[serde(default)]
    pub exclude: Vec<String>,
}

/// Weights for each analysis pass (higher = more impact on grade).
#[derive(Debug, Clone, Deserialize, Serialize, Default, Merge, schemars::JsonSchema)]
#[serde(default)]
pub struct AnalyzeWeights {
    pub health: Option<f64>,
    pub complexity: Option<f64>,
    pub security: Option<f64>,
    pub duplicate_functions: Option<f64>,
}

impl AnalyzeWeights {
    pub fn health(&self) -> f64 {
        self.health.unwrap_or(1.0)
    }
    pub fn complexity(&self) -> f64 {
        self.complexity.unwrap_or(0.5)
    }
    pub fn security(&self) -> f64 {
        self.security.unwrap_or(2.0)
    }
    pub fn duplicate_functions(&self) -> f64 {
        self.duplicate_functions.unwrap_or(0.3)
    }
}

impl AnalyzeConfig {
    pub fn threshold(&self) -> Option<usize> {
        self.threshold
    }

    pub fn compact(&self) -> bool {
        self.compact.unwrap_or(false)
    }

    pub fn health(&self) -> bool {
        self.health.unwrap_or(true)
    }

    pub fn complexity(&self) -> bool {
        self.complexity.unwrap_or(true)
    }

    pub fn security(&self) -> bool {
        self.security.unwrap_or(true)
    }

    pub fn duplicate_functions(&self) -> bool {
        self.duplicate_functions.unwrap_or(false)
    }

    pub fn weights(&self) -> AnalyzeWeights {
        self.weights.clone().unwrap_or_default()
    }

    pub fn exclude_interface_impls(&self) -> bool {
        self.exclude_interface_impls.unwrap_or(true)
    }
}

/// Load patterns from a .normalize allow file (e.g., hotspots-allow, large-files-allow)
pub fn load_allow_file(root: &Path, filename: &str) -> Vec<String> {
    let path = root.join(".normalize").join(filename);
    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };

    content
        .lines()
        .filter_map(|line| {
            // Strip trailing comments
            let without_comment = line.split('#').next().unwrap_or(line);
            let trimmed = without_comment.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        })
        .collect()
}

/// Collect source code files (`.py` and `.rs`) from `root`, applying the optional filter.
///
/// Shared by complexity and length analysis passes.
pub(super) fn collect_code_files<'a>(
    all_files: &'a [normalize_path_resolve::PathMatch],
    filter: Option<&Filter>,
) -> Vec<&'a normalize_path_resolve::PathMatch> {
    all_files
        .iter()
        .filter(|f| {
            f.kind == "file" && normalize_languages::support_for_path(Path::new(&f.path)).is_some()
        })
        .filter(|f| {
            filter
                .map(|flt| flt.matches(Path::new(&f.path)))
                .unwrap_or(true)
        })
        .collect()
}

/// Check if a path is a source file we can analyze.
pub(crate) fn is_source_file(path: &Path) -> bool {
    !is_generated_file(path) && normalize_languages::support_for_path(path).is_some()
}

/// Known generated/lockfiles that are not useful to analyze for code quality.
fn is_generated_file(path: &Path) -> bool {
    let file_name = match path.file_name().and_then(|n| n.to_str()) {
        Some(n) => n,
        None => return false,
    };
    // Common lock files by exact name
    matches!(
        file_name,
        "package-lock.json"
            | "yarn.lock"
            | "pnpm-lock.yaml"
            | "bun.lockb"
            | "Cargo.lock"
            | "composer.lock"
            | "Gemfile.lock"
            | "poetry.lock"
            | "Pipfile.lock"
            | "packages.lock.json"
    ) || file_name.ends_with(".lock")
}
