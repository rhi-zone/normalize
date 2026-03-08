//! Shared rule configuration types for all normalize rule engines.
//!
//! Both syntax rules and fact rules use `RulesConfig` as their configuration type,
//! loaded from `[rules]` in `.normalize/config.toml`.

use std::collections::HashMap;

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
}

impl normalize_core::Merge for RuleOverride {
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
