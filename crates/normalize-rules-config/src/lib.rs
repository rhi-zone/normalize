//! Shared rule configuration types for all normalize rule engines.
//!
//! Both syntax rules and fact rules use `RulesConfig` as their configuration type,
//! loaded from `[rules]` in `.normalize/config.toml`.

use std::collections::HashMap;

/// Severity level for rule findings.
///
/// Shared across all rule engines (syntax, fact, native). `DiagnosticLevel` in
/// `normalize-facts-rules-api` is the ABI-stable counterpart.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Error,
    #[default]
    Warning,
    Info,
    Hint,
}

impl std::fmt::Display for Severity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Severity::Error => write!(f, "error"),
            Severity::Warning => write!(f, "warning"),
            Severity::Info => write!(f, "info"),
            Severity::Hint => write!(f, "hint"),
        }
    }
}

impl std::str::FromStr for Severity {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "error" => Ok(Severity::Error),
            "warning" | "warn" => Ok(Severity::Warning),
            "info" | "note" => Ok(Severity::Info),
            "hint" => Ok(Severity::Hint),
            _ => Err(format!("unknown severity: {}", s)),
        }
    }
}

/// An external tool that emits SARIF 2.1.0 output (used with `--engine sarif`).
///
/// Configured via `[[rules.sarif-tools]]` in `.normalize/config.toml`.
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, Default, schemars::JsonSchema)]
#[serde(default)]
pub struct SarifTool {
    /// Display name for this tool (used as `source` in DiagnosticsReport).
    pub name: String,
    /// Command to run. `{root}` is replaced with the project root path.
    /// Example: `["npx", "eslint", "--format", "json", "{root}"]`
    pub command: Vec<String>,
}

/// Per-rule configuration override.
///
/// Used under `[rules."rule-id"]` in `.normalize/config.toml`.
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, Default, schemars::JsonSchema)]
#[serde(default)]
pub struct RuleOverride {
    /// Override the rule's severity (error, warning, info, hint).
    pub severity: Option<String>,
    /// Enable or disable the rule.
    pub enabled: Option<bool>,
    /// Additional file patterns to allow (skip) for this rule.
    #[serde(default)]
    pub allow: Vec<String>,
    /// Additional tags to add to this rule (appends to built-in tags).
    #[serde(default)]
    pub tags: Vec<String>,
    /// Rule-specific: filenames to require in each directory (OR semantics).
    ///
    /// Used by `stale-summary` and `missing-summary`. A directory is compliant
    /// if it contains **any** of the listed files. Defaults to `["SUMMARY.md"]`
    /// when not set. To require both files, use two `[[rule]]` entries instead.
    ///
    /// Accepts either a single string (`filename = "CLAUDE.md"`) or a list
    /// (`filenames = ["SUMMARY.md", "CLAUDE.md"]`). The single-string form is
    /// deserialized as a one-element list.
    #[serde(default, deserialize_with = "deserialize_filenames")]
    pub filenames: Vec<String>,
}

fn deserialize_filenames<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::Deserialize as _;

    #[derive(serde::Deserialize)]
    #[serde(untagged)]
    enum OneOrMany {
        One(String),
        Many(Vec<String>),
    }

    match OneOrMany::deserialize(deserializer)? {
        OneOrMany::One(s) => Ok(vec![s]),
        OneOrMany::Many(v) => Ok(v),
    }
}

impl normalize_core::Merge for RuleOverride {
    /// Merge two `RuleOverride` values, with `other` taking priority.
    ///
    /// - `Option` fields: `other`'s value wins if `Some`; falls back to `self`.
    /// - Vec fields (`allow`, `tags`, `filenames`): if `other`'s field is non-empty
    ///   it replaces `self`'s field entirely; an empty `other` field inherits from
    ///   `self`. **This means you cannot reset a Vec to empty via merge** — an empty
    ///   `other` vec is treated as "no override" rather than "clear the list".
    fn merge(self, other: Self) -> Self {
        Self {
            severity: other.severity.or(self.severity),
            enabled: other.enabled.or(self.enabled),
            allow: if other.allow.is_empty() {
                self.allow
            } else {
                other.allow
            },
            tags: if other.tags.is_empty() {
                self.tags
            } else {
                other.tags
            },
            filenames: if other.filenames.is_empty() {
                self.filenames
            } else {
                other.filenames
            },
        }
    }
}

/// Rules configuration covering all engines (syntax, fact, native, sarif).
///
/// Deserialized from `[rules]` in `.normalize/config.toml`.
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, Default, schemars::JsonSchema)]
#[serde(default)]
pub struct RulesConfig {
    /// Allow patterns applied to every rule (e.g. `["**/tests/fixtures/**"]`).
    /// Entries here skip violations in matching files across all rules.
    #[serde(rename = "global-allow")]
    pub global_allow: Vec<String>,
    /// External tools that emit SARIF 2.1.0 output (the `sarif` engine).
    #[serde(rename = "sarif-tools")]
    pub sarif_tools: Vec<SarifTool>,
    /// Per-rule configuration overrides, keyed by rule ID.
    #[serde(flatten)]
    pub rules: HashMap<String, RuleOverride>,
}

impl normalize_core::Merge for RulesConfig {
    /// Merge two `RulesConfig` values, with `other` taking priority.
    ///
    /// - Vec fields (`global_allow`, `sarif_tools`): if `other`'s field is non-empty
    ///   it replaces `self`'s field; an empty `other` field inherits from `self`.
    ///   **This means you cannot reset a Vec to empty via merge** — an empty `other`
    ///   vec is treated as "no override" rather than "clear the list".
    /// - `rules` HashMap: merged using `extend`, so `other`'s keys override `self`'s
    ///   keys. Keys present only in `self` are preserved.
    fn merge(self, other: Self) -> Self {
        let global_allow = if other.global_allow.is_empty() {
            self.global_allow
        } else {
            other.global_allow
        };
        let sarif_tools = if other.sarif_tools.is_empty() {
            self.sarif_tools
        } else {
            other.sarif_tools
        };
        let mut merged_rules = self.rules;
        merged_rules.extend(other.rules);
        Self {
            global_allow,
            sarif_tools,
            rules: merged_rules,
        }
    }
}
