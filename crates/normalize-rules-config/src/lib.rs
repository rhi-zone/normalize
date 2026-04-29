//! Shared rule configuration types for all normalize rule engines.
//!
//! Both syntax rules and fact rules use `RulesConfig` as their configuration type,
//! loaded from `[rules]` in `.normalize/config.toml`.

use std::collections::{HashMap, HashSet};
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
    /// Glob patterns (relative to project root) for files this tool watches.
    ///
    /// When set, `normalize rules run` caches this tool's SARIF output keyed by the
    /// maximum mtime of all matching files. On warm runs where no watched file has
    /// changed, the tool is skipped and results are served from cache.
    ///
    /// If empty (the default), the tool always re-runs (no caching).
    ///
    /// Example: `["**/*.py"]` for a Python linter, `["**/*.ts"]` for a TypeScript checker.
    #[serde(default)]
    pub watch: Vec<String>,
}

/// Common per-rule configuration fields shared across all rule engines.
///
/// Used under `[rules.rule."rule-id"]` in `.normalize/config.toml`. These fields
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
/// Deserialized from `[rules]` in `.normalize/config.toml`. Per-rule overrides
/// live under `[rules.rule."<id>"]`. Engine-wide settings live as bare keys
/// directly under `[rules]` (e.g. `global-allow`, `sarif-tools`).
///
/// **Legacy layout** (`[rules."<id>"]` directly under `[rules]`) is still parsed
/// for one release with a stderr deprecation warning. It is unsound in principle
/// because a rule named `global-allow` would collide with the engine-wide key —
/// the new nested layout removes the namespace collision.
#[derive(Debug, Clone, serde::Serialize, Default, schemars::JsonSchema)]
pub struct RulesConfig {
    /// Allow patterns applied to every rule (e.g. `["**/tests/fixtures/**"]`).
    /// Entries here skip violations in matching files across all rules.
    #[serde(
        rename = "global-allow",
        default,
        skip_serializing_if = "Vec::is_empty"
    )]
    pub global_allow: Vec<String>,
    /// External tools that emit SARIF 2.1.0 output (the `sarif` engine).
    #[serde(rename = "sarif-tools", default, skip_serializing_if = "Vec::is_empty")]
    pub sarif_tools: Vec<SarifTool>,
    /// Per-rule configuration overrides, keyed by rule ID.
    ///
    /// Serialized under the `rule` sub-table (`[rules.rule."<id>"]`). On
    /// deserialization, both the new nested layout and the legacy flat layout
    /// (`[rules."<id>"]`) are accepted; the legacy form emits a stderr
    /// deprecation warning.
    #[serde(default, rename = "rule", skip_serializing_if = "HashMap::is_empty")]
    pub rules: HashMap<String, RuleOverride>,
}

/// Engine-wide bare keys reserved under `[rules]`. Anything else found at the
/// top level of `[rules]` is interpreted as a legacy `[rules."<id>"]` per-rule
/// override (and triggers a deprecation warning).
const RULES_RESERVED_KEYS: &[&str] = &["global-allow", "sarif-tools", "rule"];

impl<'de> serde::Deserialize<'de> for RulesConfig {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        // Capture every entry as a generic toml::Value first so we can route bare
        // engine keys, the new `rule` sub-table, and legacy per-rule entries
        // (`[rules."<id>"]`) separately.
        let raw: HashMap<String, toml::Value> = HashMap::deserialize(deserializer)?;

        let mut global_allow: Vec<String> = Vec::new();
        let mut sarif_tools: Vec<SarifTool> = Vec::new();
        let mut rules: HashMap<String, RuleOverride> = HashMap::new();
        let mut legacy_rule_ids: Vec<String> = Vec::new();

        for (key, value) in raw {
            match key.as_str() {
                "global-allow" => {
                    global_allow = value.try_into().map_err(serde::de::Error::custom)?;
                }
                "sarif-tools" => {
                    sarif_tools = value.try_into().map_err(serde::de::Error::custom)?;
                }
                "rule" => {
                    let nested: HashMap<String, RuleOverride> =
                        value.try_into().map_err(serde::de::Error::custom)?;
                    // Nested-layout entries take precedence over any legacy entries
                    // with the same id (extend overwrites).
                    rules.extend(nested);
                }
                _ => {
                    // Legacy: bare key is a rule id ([rules."<id>"]).
                    let override_: RuleOverride =
                        value.try_into().map_err(serde::de::Error::custom)?;
                    legacy_rule_ids.push(key.clone());
                    // Don't overwrite a nested entry with the same id if one
                    // already landed; nested wins.
                    rules.entry(key).or_insert(override_);
                }
            }
        }

        if !legacy_rule_ids.is_empty() {
            legacy_rule_ids.sort();
            eprintln!(
                "warning: deprecated [rules.\"<id>\"] layout in .normalize/config.toml — \
                 migrate to [rules.rule.\"<id>\"] (affected rule ids: {}). \
                 The legacy layout will be removed in a future release.",
                legacy_rule_ids.join(", "),
            );
        }

        // Sanity check: forbid any future engine key colliding with the
        // reserved bare-key namespace from being interpreted as a rule.
        // (Currently RULES_RESERVED_KEYS is only used for documentation /
        // future-proofing — every reserved key is already handled above.)
        let _ = RULES_RESERVED_KEYS;

        Ok(RulesConfig {
            global_allow,
            sarif_tools,
            rules,
        })
    }
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
    /// Additional gitignore-style patterns to always skip. Default: `[".git"]`.
    ///
    /// Patterns use the same syntax as `.gitignore`:
    /// - A pattern with no slash (e.g. `node_modules`, `.git`) matches any
    ///   directory or file with that basename, at any depth.
    /// - A pattern with a slash (e.g. `crates/foo/build/`, `**/target/`) is
    ///   anchored relative to the project root.
    /// - Trailing `/` restricts the match to directories.
    /// - `**` matches any number of path segments.
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

    /// Compile the configured `exclude` patterns into a gitignore matcher anchored at `root`.
    ///
    /// Returns an empty matcher if no patterns are configured. Invalid patterns
    /// are silently dropped (consistent with how `.gitignore` itself behaves).
    pub fn compiled_excludes(&self, root: &Path) -> ignore::gitignore::Gitignore {
        let mut builder = ignore::gitignore::GitignoreBuilder::new(root);
        for pat in self.exclude() {
            // GitignoreBuilder::add_line silently no-ops on bad patterns.
            let _ = builder.add_line(None, pat);
        }
        builder.build().unwrap_or_else(|_| {
            // Fallback: empty matcher (matches nothing).
            ignore::gitignore::Gitignore::empty()
        })
    }

    /// Check whether a path (relative to `root`) matches any exclude pattern.
    ///
    /// `is_dir` distinguishes directories from files (relevant for trailing-`/` patterns).
    /// For repeat queries, prefer building [`compiled_excludes`] once and querying
    /// it directly; this method is a convenience for one-shot checks.
    pub fn is_excluded_path(&self, root: &Path, rel_path: &Path, is_dir: bool) -> bool {
        let gi = self.compiled_excludes(root);
        gi.matched_path_or_any_parents(rel_path, is_dir).is_ignore()
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

/// Surgical-invalidation diff between two rule configurations.
///
/// Produced by [`ConfigDiff::compute`] to classify what changed between an old
/// `(RulesConfig, WalkConfig)` snapshot and a new one. Consumers (the daemon's
/// config-reload handler) use the classification to pick the cheapest correct
/// invalidation strategy:
///
/// - **Tier 1 (filter-only):** severities, allow-lists, or `enabled = false`
///   changed. The cached findings are still correct — applying the new config's
///   filter at serve time is enough. No re-evaluation.
/// - **Tier 2 (per-rule re-run):** specific rules' behavior changed (newly
///   enabled, rule-specific config field, or backing `.scm` file edited).
///   Only those rules need to be re-evaluated; everything else stays cached.
/// - **Tier 3 (full reprime):** `[walk] exclude` changed (file set differs) or
///   the diff doesn't fit the above. Conservative fallback.
///
/// `.scm` rule-definition file diffs are tracked outside this struct because
/// this crate has no filesystem dependency; the daemon hashes
/// `.normalize/rules/**` itself and unions its result into `rules_to_rerun`
/// before consulting [`ConfigDiff::is_filter_only`] / [`ConfigDiff::requires_full_reprime`].
#[derive(Debug, Default, Clone)]
pub struct ConfigDiff {
    /// Rules whose evaluation behavior changed (newly-enabled, rule-specific
    /// config field, or `.scm` definition edited). These need to be re-run.
    pub rules_to_rerun: HashSet<String>,
    /// Rules that became disabled. Their cached findings should be dropped at
    /// serve time (no re-run needed).
    pub rules_disabled: HashSet<String>,
    /// True if any allow-list (per-rule `allow` or top-level `global-allow`)
    /// changed without a corresponding behavior change. Filter at serve time.
    pub allow_lists_changed: bool,
    /// True if any rule's severity changed without a corresponding behavior
    /// change. Override severity at serve time.
    pub severities_changed: bool,
    /// True if `[walk] exclude` changed. Forces a full reprime (Tier 3) because
    /// the file set may differ.
    pub walk_exclude_changed: bool,
}

impl ConfigDiff {
    /// Compute a diff describing what changed between `old` and `new`.
    ///
    /// The diff classifies each per-rule change into the cheapest tier that's
    /// still correct. Adding/removing a rule entry that flips `enabled` from
    /// the implicit default is treated the same as toggling it explicitly.
    pub fn compute(
        old_rules: &RulesConfig,
        new_rules: &RulesConfig,
        old_walk: &WalkConfig,
        new_walk: &WalkConfig,
    ) -> Self {
        let mut diff = ConfigDiff::default();

        // Walk-exclude changed → Tier 3.
        if old_walk.exclude() != new_walk.exclude() {
            diff.walk_exclude_changed = true;
        }

        // Global allow change → filter-only.
        if old_rules.global_allow != new_rules.global_allow {
            diff.allow_lists_changed = true;
        }

        // Walk every rule id present in either snapshot.
        let ids: HashSet<&str> = old_rules
            .rules
            .keys()
            .chain(new_rules.rules.keys())
            .map(String::as_str)
            .collect();

        for id in ids {
            let old = old_rules.rules.get(id);
            let new = new_rules.rules.get(id);

            // Enabled-state transitions. `None` ≡ default-enabled, so missing
            // entry vs `enabled = Some(true)` is *not* a state change.
            let was_enabled = old.is_none_or(|o| o.enabled.unwrap_or(true));
            let is_enabled = new.is_none_or(|n| n.enabled.unwrap_or(true));
            match (was_enabled, is_enabled) {
                (true, false) => {
                    diff.rules_disabled.insert(id.to_string());
                }
                (false, true) => {
                    // Newly enabled — must re-evaluate.
                    diff.rules_to_rerun.insert(id.to_string());
                }
                _ => {}
            }

            // Severity change is filter-only; only flag it when the rule is
            // (and stays) enabled — otherwise the disabled/re-enabled paths
            // already handle it.
            if was_enabled && is_enabled {
                let old_sev = old.and_then(|o| o.severity.as_deref());
                let new_sev = new.and_then(|n| n.severity.as_deref());
                if old_sev != new_sev {
                    diff.severities_changed = true;
                }

                // Per-rule allow-list change is filter-only.
                let old_allow = old.map(|o| o.allow.as_slice()).unwrap_or(&[]);
                let new_allow = new.map(|n| n.allow.as_slice()).unwrap_or(&[]);
                if old_allow != new_allow {
                    diff.allow_lists_changed = true;
                }

                // Rule-specific config (the `extra` toml table) or `tags`
                // changed → behavior changed → re-run the rule.
                let old_extra = old.map(|o| &o.extra);
                let new_extra = new.map(|n| &n.extra);
                if old_extra != new_extra {
                    diff.rules_to_rerun.insert(id.to_string());
                }
                let old_tags = old.map(|o| o.tags.as_slice()).unwrap_or(&[]);
                let new_tags = new.map(|n| n.tags.as_slice()).unwrap_or(&[]);
                if old_tags != new_tags {
                    // Tags affect filter selection (`--tag`) but do not change
                    // findings under a typical `rules run`. Treat as filter-only.
                    diff.allow_lists_changed = true;
                }
            }
        }

        // sarif-tools change → conservatively force a re-run of every sarif
        // tool by listing them in `rules_to_rerun` keyed by tool name. The
        // daemon today doesn't run sarif tools through the per-rule re-eval
        // path, so this surfaces as "non-filter-only" → full reprime, which
        // is the conservative correct behavior.
        if old_rules.sarif_tools.len() != new_rules.sarif_tools.len() {
            diff.rules_to_rerun
                .insert("__sarif_tools_changed__".to_string());
        } else {
            for (a, b) in old_rules
                .sarif_tools
                .iter()
                .zip(new_rules.sarif_tools.iter())
            {
                if a.name != b.name || a.command != b.command || a.watch != b.watch {
                    diff.rules_to_rerun
                        .insert("__sarif_tools_changed__".to_string());
                    break;
                }
            }
        }

        diff
    }

    /// True if this diff can be honored by re-filtering cached findings at
    /// serve time, with no re-evaluation.
    ///
    /// Specifically: no rule needs re-running and `[walk] exclude` is
    /// unchanged. Allow-list, severity, and `enabled = false` changes are all
    /// filter-only because the cached findings are a superset of the new
    /// answer — dropping disabled rules / allow-matched paths and overriding
    /// severities at serve time produces the correct result.
    pub fn is_filter_only(&self) -> bool {
        self.rules_to_rerun.is_empty() && !self.walk_exclude_changed
    }

    /// True if this diff requires a full reprime (Tier 3).
    ///
    /// Today only `walk_exclude_changed` triggers this; future fields that
    /// can't be expressed as either filter-only or per-rule re-run should
    /// extend this check.
    pub fn requires_full_reprime(&self) -> bool {
        self.walk_exclude_changed
    }

    /// True if this diff has no observable effect.
    pub fn is_empty(&self) -> bool {
        self.rules_to_rerun.is_empty()
            && self.rules_disabled.is_empty()
            && !self.allow_lists_changed
            && !self.severities_changed
            && !self.walk_exclude_changed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allow_field_not_swallowed_by_extra() {
        let toml_str = r#"
global-allow = ["**/fixtures/**"]

[rule."no-grammar-loader-new"]
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
        // Simulate the actual config.toml structure (new layout).
        let toml_str = r#"
global-allow = ["**/tests/fixtures/**", "**/fixtures/**", ".claude/**"]

[rule."rust/dbg-macro"]
severity = "error"
allow = ["**/tests/fixtures/**"]

[rule."no-grammar-loader-new"]
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
    fn legacy_layout_still_parses() {
        // Old layout: per-rule entries directly under [rules]. Should still load
        // (with a stderr deprecation warning) for one release.
        let toml_str = r#"
global-allow = ["**/fixtures/**"]

["rust/dbg-macro"]
severity = "error"
"#;
        let config: RulesConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.global_allow, vec!["**/fixtures/**"]);
        let dbg = config.rules.get("rust/dbg-macro").unwrap();
        assert_eq!(dbg.severity.as_deref(), Some("error"));
    }

    #[test]
    fn nested_layout_does_not_collide_with_engine_keys() {
        // A rule literally named "global-allow" must coexist with the
        // engine-wide global-allow value — only possible because per-rule
        // configs live under the `rule` sub-table.
        let toml_str = r#"
global-allow = ["**/fixtures/**"]

[rule."global-allow"]
severity = "error"
allow = ["legacy/**"]
"#;
        let config: RulesConfig = toml::from_str(toml_str).unwrap();
        // Engine-wide value preserved
        assert_eq!(config.global_allow, vec!["**/fixtures/**"]);
        // Per-rule override for the (admittedly weird) rule named "global-allow"
        let r = config.rules.get("global-allow").unwrap();
        assert_eq!(r.severity.as_deref(), Some("error"));
        assert_eq!(r.allow, vec!["legacy/**"]);
    }

    #[test]
    fn nested_layout_wins_over_legacy_on_id_collision() {
        // If both layouts define the same rule id, the new layout wins.
        let toml_str = r#"
[rule."rust/dbg-macro"]
severity = "warning"

["rust/dbg-macro"]
severity = "error"
"#;
        let config: RulesConfig = toml::from_str(toml_str).unwrap();
        let dbg = config.rules.get("rust/dbg-macro").unwrap();
        assert_eq!(dbg.severity.as_deref(), Some("warning"));
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
        let root = Path::new("/tmp/root");
        assert!(config.is_excluded_path(root, Path::new(".git"), true));
        assert!(!config.is_excluded_path(root, Path::new("src"), true));
    }

    #[test]
    fn walk_config_custom() {
        let config = WalkConfig {
            ignore_files: Some(vec![".gitignore".into(), ".npmignore".into()]),
            exclude: Some(vec![".git".into(), "node_modules".into()]),
        };
        assert_eq!(config.ignore_files(), vec![".gitignore", ".npmignore"]);
        assert_eq!(config.exclude(), vec![".git", "node_modules"]);
        let root = Path::new("/tmp/root");
        assert!(config.is_excluded_path(root, Path::new("node_modules"), true));
        assert!(!config.is_excluded_path(root, Path::new("src"), true));
    }

    #[test]
    fn walk_config_empty_disables() {
        let config = WalkConfig {
            ignore_files: Some(vec![]),
            exclude: Some(vec![]),
        };
        assert!(config.ignore_files().is_empty());
        assert!(config.exclude().is_empty());
        let root = Path::new("/tmp/root");
        assert!(!config.is_excluded_path(root, Path::new(".git"), true));
    }

    #[test]
    fn walk_config_excludes_basename_at_any_depth() {
        // gitignore semantics: pattern with no slash matches at any depth.
        let config = WalkConfig {
            ignore_files: None,
            exclude: Some(vec!["node_modules".into(), "worktrees".into()]),
        };
        let root = Path::new("/tmp/root");
        // Top-level
        assert!(config.is_excluded_path(root, Path::new("node_modules"), true));
        // Nested
        assert!(config.is_excluded_path(root, Path::new("crates/foo/node_modules"), true));
        // .claude/worktrees nested
        assert!(config.is_excluded_path(root, Path::new(".claude/worktrees"), true));
    }

    #[test]
    fn walk_config_excludes_anchored_glob() {
        let config = WalkConfig {
            ignore_files: None,
            exclude: Some(vec!["**/target/".into(), "path/to/specific.rs".into()]),
        };
        let root = Path::new("/tmp/root");
        assert!(config.is_excluded_path(root, Path::new("crates/foo/target"), true));
        assert!(config.is_excluded_path(root, Path::new("target"), true));
        assert!(config.is_excluded_path(root, Path::new("path/to/specific.rs"), false));
        assert!(!config.is_excluded_path(root, Path::new("path/to/other.rs"), false));
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

    // -- ConfigDiff -----------------------------------------------------------

    fn parse_rules(s: &str) -> RulesConfig {
        toml::from_str(s).unwrap()
    }

    #[test]
    fn config_diff_no_change_is_empty() {
        let cfg = parse_rules(
            r#"
[rule."rust/dbg-macro"]
severity = "error"
"#,
        );
        let walk = WalkConfig::default();
        let diff = ConfigDiff::compute(&cfg, &cfg, &walk, &walk);
        assert!(diff.is_empty());
        assert!(diff.is_filter_only());
        assert!(!diff.requires_full_reprime());
    }

    #[test]
    fn config_diff_severity_only_is_filter_only() {
        let old = parse_rules(
            r#"
[rule."rust/dbg-macro"]
severity = "error"
"#,
        );
        let new = parse_rules(
            r#"
[rule."rust/dbg-macro"]
severity = "info"
"#,
        );
        let walk = WalkConfig::default();
        let diff = ConfigDiff::compute(&old, &new, &walk, &walk);
        assert!(diff.severities_changed);
        assert!(diff.is_filter_only());
        assert!(diff.rules_to_rerun.is_empty());
    }

    #[test]
    fn config_diff_allow_change_is_filter_only() {
        let old = parse_rules(
            r#"
global-allow = ["**/fixtures/**"]
"#,
        );
        let new = parse_rules(
            r#"
global-allow = ["**/fixtures/**", "**/tests/**"]
"#,
        );
        let walk = WalkConfig::default();
        let diff = ConfigDiff::compute(&old, &new, &walk, &walk);
        assert!(diff.allow_lists_changed);
        assert!(diff.is_filter_only());
    }

    #[test]
    fn config_diff_disable_is_filter_only() {
        let old = parse_rules(
            r#"
[rule."rust/dbg-macro"]
severity = "error"
"#,
        );
        let new = parse_rules(
            r#"
[rule."rust/dbg-macro"]
severity = "error"
enabled = false
"#,
        );
        let walk = WalkConfig::default();
        let diff = ConfigDiff::compute(&old, &new, &walk, &walk);
        assert!(diff.rules_disabled.contains("rust/dbg-macro"));
        assert!(diff.rules_to_rerun.is_empty());
        assert!(diff.is_filter_only());
    }

    #[test]
    fn config_diff_enable_requires_rerun() {
        let old = parse_rules(
            r#"
[rule."rust/dbg-macro"]
enabled = false
"#,
        );
        let new = parse_rules(
            r#"
[rule."rust/dbg-macro"]
enabled = true
"#,
        );
        let walk = WalkConfig::default();
        let diff = ConfigDiff::compute(&old, &new, &walk, &walk);
        assert!(diff.rules_to_rerun.contains("rust/dbg-macro"));
        assert!(!diff.is_filter_only());
        assert!(!diff.requires_full_reprime());
    }

    #[test]
    fn config_diff_threshold_change_requires_rerun() {
        let old = parse_rules(
            r#"
[rule."long-function"]
threshold = 100
"#,
        );
        let new = parse_rules(
            r#"
[rule."long-function"]
threshold = 50
"#,
        );
        let walk = WalkConfig::default();
        let diff = ConfigDiff::compute(&old, &new, &walk, &walk);
        assert!(diff.rules_to_rerun.contains("long-function"));
    }

    #[test]
    fn config_diff_walk_exclude_change_requires_full_reprime() {
        let cfg = RulesConfig::default();
        let old_walk = WalkConfig::default();
        let new_walk = WalkConfig {
            ignore_files: None,
            exclude: Some(vec![".git".into(), "node_modules".into()]),
        };
        let diff = ConfigDiff::compute(&cfg, &cfg, &old_walk, &new_walk);
        assert!(diff.walk_exclude_changed);
        assert!(diff.requires_full_reprime());
        assert!(!diff.is_filter_only());
    }
}
