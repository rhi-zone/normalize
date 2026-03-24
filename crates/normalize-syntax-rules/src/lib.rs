//! Syntax-based linting with tree-sitter queries.
//!
//! This crate provides:
//! - Rule loading from multiple sources (builtins, user global, project)
//! - Rule execution with combined query optimization
//! - Pluggable data sources for rule conditionals
//!
//! # Rule File Format
//!
//! ```scm
//! # ---
//! # id = "no-unwrap"
//! # severity = "warning"
//! # message = "Avoid unwrap() on user input"
//! # allow = ["**/tests/**"]
//! # requires = { "rust.edition" = ">=2024" }
//! # enabled = true  # set to false to disable a builtin
//! # fix = ""  # empty = delete match, or use "$capture" to substitute
//! # ---
//!
//! (call_expression
//!   function: (field_expression
//!     field: (field_identifier) @method)
//!   (#eq? @method "unwrap")) @match
//! ```

mod builtin;
mod loader;
pub mod query;
mod runner;
mod sources;

#[cfg(feature = "cli")]
pub mod service;

pub use builtin::BUILTIN_RULES;
pub use loader::{RuleOverride, RulesConfig, load_all_rules, parse_rule_content};
pub use query::{MatchResult, is_sexp_pattern, run_astgrep_query, run_sexp_query};
pub use runner::{DebugFlags, Finding, apply_fixes, evaluate_predicates, run_rules};
pub use sources::{
    EnvSource, GitSource, GoSource, PathSource, PythonSource, RuleSource, RustSource,
    SourceContext, SourceRegistry, TypeScriptSource, builtin_registry,
};

/// Severity level for rule findings. Defined in normalize-rules-config for sharing
/// across all rule engines (syntax, fact).
pub use normalize_rules_config::Severity;

use glob::Pattern;
use std::collections::HashMap;
use std::path::PathBuf;

/// A syntax rule definition.
#[derive(Debug)]
pub struct Rule {
    /// Unique identifier for this rule.
    pub id: String,
    /// The tree-sitter query pattern.
    pub query_str: String,
    /// Severity level.
    pub severity: Severity,
    /// Message to display when the rule matches.
    pub message: String,
    /// Glob patterns for files where matches are allowed.
    pub allow: Vec<Pattern>,
    /// Source file path of this rule (empty for builtins).
    pub source_path: PathBuf,
    /// Languages this rule applies to (inferred from query or explicit).
    pub languages: Vec<String>,
    /// Whether this rule is enabled.
    pub enabled: bool,
    /// Whether this is a builtin rule.
    pub builtin: bool,
    /// Conditions that must be met for this rule to apply.
    /// Format: { "namespace.key" = "value" } or { "namespace.key" = ">=value" }
    pub requires: HashMap<String, String>,
    /// Auto-fix template using capture names from the query.
    ///
    /// Substitution syntax: `$capture_name` is replaced by the text of the named capture
    /// (e.g. `$fn_name`), and `$match` is replaced by the entire matched node's text.
    /// An empty string means "delete the matched node entirely".
    /// `None` means the rule has no auto-fix.
    pub fix: Option<String>,
    /// Tags for grouping and filtering rules by concept (e.g. "debug-print", "security").
    pub tags: Vec<String>,
    /// Documentation from the markdown comment block between frontmatter and query.
    pub doc: Option<String>,
    /// Whether this rule is recommended for most projects (catches real bugs, not style).
    pub recommended: bool,
}

/// A builtin rule definition (id, content).
pub struct BuiltinRule {
    pub id: &'static str,
    pub content: &'static str,
}
