//! Shared rule configuration types for all normalize rule engines.
//!
//! Both syntax rules and fact rules use `RulesConfig` as their configuration type,
//! loaded from `[rules]` in `.normalize/config.toml`.

use std::collections::HashMap;
use std::path::Path;

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

/// Common per-rule configuration fields shared across all rule engines.
///
/// Used under `[rules."rule-id"]` in `.normalize/config.toml`. These fields
/// apply to every rule regardless of engine. Rule-specific configuration
/// (e.g. thresholds, filenames) is defined as typed structs owned by each
/// rule and deserialized from the same TOML table via `#[serde(flatten)]`.
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
    /// Raw TOML table for rule-specific fields. Each rule deserializes its
    /// own typed config from this via [`RuleOverride::rule_config`].
    #[serde(flatten)]
    #[schemars(skip)]
    pub extra: std::collections::HashMap<String, toml::Value>,
}

pub fn deserialize_one_or_many<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
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
    /// - Vec fields (`allow`, `tags`): if `other`'s field is non-empty it replaces
    ///   `self`'s field entirely; an empty `other` field inherits from `self`.
    /// - `extra` HashMap: merged key-by-key, `other`'s keys override `self`'s.
    fn merge(self, other: Self) -> Self {
        let mut extra = self.extra;
        extra.extend(other.extra);
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
            extra,
        }
    }
}

impl RuleOverride {
    /// Deserialize rule-specific config from the `extra` fields.
    ///
    /// Each rule defines a typed config struct and calls this to extract it.
    /// Unknown fields in `extra` that don't match `T`'s fields are ignored.
    ///
    /// ```ignore
    /// #[derive(Deserialize, Default)]
    /// struct LargeFileConfig { threshold: Option<u64> }
    ///
    /// let cfg: LargeFileConfig = override_.rule_config();
    /// let threshold = cfg.threshold.unwrap_or(500);
    /// ```
    pub fn rule_config<T: serde::de::DeserializeOwned + Default>(&self) -> T {
        let table = toml::Value::Table(
            self.extra
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect(),
        );
        table.try_into().unwrap_or_default()
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

/// Configuration for directory walking behavior.
///
/// Controls which ignore files are respected and which directories are always
/// excluded. Deserialized from `[walk]` in `.normalize/config.toml`.
///
/// ```toml
/// [walk]
/// ignore_files = [".gitignore"]   # default
/// exclude = [".git"]              # default
/// ```
#[derive(
    Debug,
    Clone,
    serde::Deserialize,
    serde::Serialize,
    Default,
    schemars::JsonSchema,
    normalize_core::Merge,
)]
#[serde(default)]
pub struct WalkConfig {
    /// List of gitignore-format files to respect. Default: `[".gitignore"]`.
    /// Set to `[]` to disable gitignore-based exclusion entirely.
    pub ignore_files: Option<Vec<String>>,
    /// Additional directory/file name patterns to always skip. Default: `[".git"]`.
    /// Matched against directory entry file names (not full paths).
    pub exclude: Option<Vec<String>>,
}

impl WalkConfig {
    /// Returns the ignore files to respect, defaulting to `[".gitignore"]`.
    pub fn ignore_files(&self) -> Vec<&str> {
        match &self.ignore_files {
            Some(v) => v.iter().map(|s| s.as_str()).collect(),
            None => vec![".gitignore"],
        }
    }

    /// Returns the directory patterns to exclude, defaulting to `[".git"]`.
    pub fn exclude(&self) -> Vec<&str> {
        match &self.exclude {
            Some(v) => v.iter().map(|s| s.as_str()).collect(),
            None => vec![".git"],
        }
    }

    /// Check whether a directory entry's file name matches any exclude pattern.
    pub fn is_excluded(&self, file_name: &std::ffi::OsStr) -> bool {
        let name = file_name.to_string_lossy();
        self.exclude().iter().any(|pat| *pat == name.as_ref())
    }
}

/// Pre-walk path filter for `--only` / `--exclude` glob patterns.
///
/// Compiled once in the service layer and threaded to each rule engine so files
/// can be skipped *before* parsing or walking. The post-walk filter in the
/// service layer remains as a safety net.
#[derive(Debug, Clone, Default)]
pub struct PathFilter {
    pub only: Vec<glob::Pattern>,
    pub exclude: Vec<glob::Pattern>,
}

impl PathFilter {
    /// Build a `PathFilter` from raw glob strings (as provided by CLI flags).
    /// Invalid patterns are silently dropped (matches the post-walk filter behavior).
    pub fn new(only: &[String], exclude: &[String]) -> Self {
        Self {
            only: only
                .iter()
                .filter_map(|s| glob::Pattern::new(s).ok())
                .collect(),
            exclude: exclude
                .iter()
                .filter_map(|s| glob::Pattern::new(s).ok())
                .collect(),
        }
    }

    /// Returns `true` if this filter has no patterns (i.e. passes everything).
    pub fn is_empty(&self) -> bool {
        self.only.is_empty() && self.exclude.is_empty()
    }

    /// Check whether a relative path passes the filter.
    ///
    /// - If `only` is non-empty, the path must match at least one `only` pattern.
    /// - If `exclude` is non-empty, the path must not match any `exclude` pattern.
    pub fn matches(&self, rel_path: &str) -> bool {
        if !self.exclude.is_empty() && self.exclude.iter().any(|p| p.matches(rel_path)) {
            return false;
        }
        if !self.only.is_empty() && !self.only.iter().any(|p| p.matches(rel_path)) {
            return false;
        }
        true
    }

    /// Convenience: check a `Path` by converting to a string first.
    pub fn matches_path(&self, rel_path: &Path) -> bool {
        self.matches(&rel_path.to_string_lossy())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allow_field_not_swallowed_by_extra() {
        let toml_str = r#"
global-allow = ["**/fixtures/**"]

[no-grammar-loader-new]
allow = ["**/tests/**", "src/lib.rs"]
threshold = 42
"#;
        let config: RulesConfig = toml::from_str(toml_str).unwrap();
        let rule = config.rules.get("no-grammar-loader-new").unwrap();
        assert_eq!(rule.allow, vec!["**/tests/**", "src/lib.rs"]);
        assert!(!rule.extra.contains_key("allow"));
        assert!(rule.extra.contains_key("threshold"));
    }

    #[test]
    fn full_config_round_trip() {
        // Simulate the actual config.toml structure
        let toml_str = r#"
global-allow = ["**/tests/fixtures/**", "**/fixtures/**", ".claude/**"]

["rust/dbg-macro"]
severity = "error"
allow = ["**/tests/fixtures/**"]

["no-grammar-loader-new"]
allow = ["**/tests/**", "crates/*/tests/**", "**/normalize-scope/**"]
"#;
        let config: RulesConfig = toml::from_str(toml_str).unwrap();
        let dbg = config.rules.get("rust/dbg-macro").unwrap();
        assert_eq!(dbg.severity.as_deref(), Some("error"));
        assert_eq!(dbg.allow, vec!["**/tests/fixtures/**"]);

        let ngl = config.rules.get("no-grammar-loader-new").unwrap();
        assert_eq!(ngl.allow.len(), 3);
        assert_eq!(ngl.allow[2], "**/normalize-scope/**");
    }

    #[test]
    fn path_filter_empty_passes_everything() {
        let f = PathFilter::default();
        assert!(f.is_empty());
        assert!(f.matches("anything/at/all.rs"));
    }

    #[test]
    fn path_filter_only() {
        let f = PathFilter::new(&["src/**/*.rs".into()], &[]);
        assert!(f.matches("src/lib.rs"));
        assert!(f.matches("src/deep/mod.rs"));
        assert!(!f.matches("tests/integration.rs"));
    }

    #[test]
    fn path_filter_exclude() {
        let f = PathFilter::new(&[], &["**/tests/**".into()]);
        assert!(f.matches("src/lib.rs"));
        assert!(!f.matches("crates/foo/tests/bar.rs"));
    }

    #[test]
    fn path_filter_only_and_exclude() {
        let f = PathFilter::new(&["crates/**/*.rs".into()], &["**/tests/**".into()]);
        assert!(f.matches("crates/foo/src/lib.rs"));
        assert!(!f.matches("crates/foo/tests/it.rs")); // excluded
        assert!(!f.matches("src/main.rs")); // not in only
    }

    #[test]
    fn walk_config_defaults() {
        let config = WalkConfig::default();
        assert_eq!(config.ignore_files(), vec![".gitignore"]);
        assert_eq!(config.exclude(), vec![".git"]);
        assert!(config.is_excluded(std::ffi::OsStr::new(".git")));
        assert!(!config.is_excluded(std::ffi::OsStr::new("src")));
    }

    #[test]
    fn walk_config_custom() {
        let config = WalkConfig {
            ignore_files: Some(vec![".gitignore".into(), ".npmignore".into()]),
            exclude: Some(vec![".git".into(), "node_modules".into()]),
        };
        assert_eq!(config.ignore_files(), vec![".gitignore", ".npmignore"]);
        assert_eq!(config.exclude(), vec![".git", "node_modules"]);
        assert!(config.is_excluded(std::ffi::OsStr::new("node_modules")));
        assert!(!config.is_excluded(std::ffi::OsStr::new("src")));
    }

    #[test]
    fn walk_config_empty_disables() {
        let config = WalkConfig {
            ignore_files: Some(vec![]),
            exclude: Some(vec![]),
        };
        assert!(config.ignore_files().is_empty());
        assert!(config.exclude().is_empty());
        assert!(!config.is_excluded(std::ffi::OsStr::new(".git")));
    }

    #[test]
    fn walk_config_deserialize() {
        let toml_str = r#"
ignore_files = [".gitignore", ".dockerignore"]
exclude = [".git", "node_modules", ".cache"]
"#;
        let config: WalkConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.ignore_files(), vec![".gitignore", ".dockerignore"]);
        assert_eq!(config.exclude(), vec![".git", "node_modules", ".cache"]);
    }

    #[test]
    fn walk_config_merge_option_semantics() {
        use normalize_core::Merge;

        // When both are default (None), result is default
        let a = WalkConfig::default();
        let b = WalkConfig::default();
        let merged = a.merge(b);
        assert_eq!(merged.ignore_files(), vec![".gitignore"]);
        assert_eq!(merged.exclude(), vec![".git"]);

        // When self has custom and other is default (None), self wins
        let a = WalkConfig {
            ignore_files: Some(vec![".npmignore".into()]),
            exclude: Some(vec!["dist".into()]),
        };
        let b = WalkConfig::default();
        let merged = a.merge(b);
        assert_eq!(merged.ignore_files(), vec![".npmignore"]);
        assert_eq!(merged.exclude(), vec!["dist"]);

        // When other has custom, other wins
        let a = WalkConfig::default();
        let b = WalkConfig {
            ignore_files: Some(vec![".npmignore".into()]),
            exclude: None,
        };
        let merged = a.merge(b);
        assert_eq!(merged.ignore_files(), vec![".npmignore"]);
        assert_eq!(merged.exclude(), vec![".git"]); // self's None → default
    }
}
