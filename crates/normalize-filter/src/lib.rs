//! Unified alias system and filter engine for normalize.
//!
//! Aliases are registered `@property`-style declarations with a declared parse
//! mode (`syntax`) and value. They serve two purposes:
//!
//! - **Filter aliases** (`syntax = "glob"` / `"path"`): expand in `--exclude` /
//!   `--only` flags, e.g. `--exclude=@tests`.
//! - **Command aliases** (`syntax = "command"` / `"sql"`): expand at the
//!   top-level, e.g. `normalize @vocabulary`.
//!
//! Built-in aliases ship with normalize. Projects can add, override, or disable
//! aliases via `[aliases]` in `.normalize/config.toml` at any directory level
//! (inner overrides outer).

#[cfg(feature = "cli")]
pub mod service;

use ignore::gitignore::{Gitignore, GitignoreBuilder};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

// ============================================================================
// Alias types
// ============================================================================

/// Declared parse mode for an alias value -- names an established formal
/// grammar that the value is written in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "config", derive(schemars::JsonSchema))]
#[serde(rename_all = "lowercase")]
pub enum AliasSyntax {
    /// POSIX shell syntax. Tokenized via `shell-words`.
    Command,
    /// Glob pattern syntax (gitignore-style).
    Glob,
    /// SQLite SQL syntax.
    Sql,
    /// Filesystem path syntax.
    Path,
}

impl std::fmt::Display for AliasSyntax {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AliasSyntax::Command => write!(f, "command"),
            AliasSyntax::Glob => write!(f, "glob"),
            AliasSyntax::Sql => write!(f, "sql"),
            AliasSyntax::Path => write!(f, "path"),
        }
    }
}

/// Alias value -- a single string or an array of strings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "config", derive(schemars::JsonSchema))]
#[serde(untagged)]
pub enum AliasValue {
    /// A single string value (used by command / sql aliases).
    Single(String),
    /// Multiple string values (used by glob / path aliases).
    Multiple(Vec<String>),
}

impl AliasValue {
    /// Return the value as a list of strings.
    pub fn as_strings(&self) -> Vec<String> {
        match self {
            AliasValue::Single(s) => vec![s.clone()],
            AliasValue::Multiple(v) => v.clone(),
        }
    }

    /// Return true if the value is empty (disabled alias).
    pub fn is_empty(&self) -> bool {
        match self {
            AliasValue::Single(s) => s.is_empty(),
            AliasValue::Multiple(v) => v.is_empty(),
        }
    }
}

/// A single alias definition with declared syntax and value.
///
/// ```toml
/// [aliases.vocabulary]
/// syntax = "command"
/// value = 'structure query "SELECT ..."'
/// description = "Most common words in symbol names"
///
/// [aliases.tests]
/// syntax = "glob"
/// value = ["**/*test*", "**/*spec*"]
/// description = "Test files"
/// ```
///
/// The `syntax` field is optional; when omitted, it is inferred from the
/// value with a warning. Omitting `syntax` is concerning, not normal.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "config", derive(schemars::JsonSchema))]
pub struct AliasEntry {
    /// Declared parse mode (names a formal grammar).
    /// When omitted, inferred from the value heuristically (with a warning).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub syntax: Option<AliasSyntax>,
    /// The alias value.
    pub value: AliasValue,
    /// Human-readable description (optional).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

impl AliasEntry {
    /// Resolve the syntax, inferring from the value if not declared.
    pub fn resolved_syntax(&self) -> AliasSyntax {
        self.syntax.unwrap_or_else(|| infer_syntax(&self.value))
    }
}

/// Known normalize top-level subcommands for syntax inference.
const KNOWN_SUBCOMMANDS: &[&str] = &[
    "alias",
    "view",
    "grep",
    "context",
    "init",
    "update",
    "translate",
    "daemon",
    "grammars",
    "guide",
    "generate",
    "structure",
    "filter",
    "syntax",
    "package",
    "docs",
    "sessions",
    "sync",
    "tools",
    "edit",
    "analyze",
    "overview",
    "rank",
    "trend",
    "budget",
    "search",
    "cfg",
    "kg",
    "ratchet",
    "rules",
    "serve",
    "similarity",
    "graph",
    "history",
    "ci",
    "config",
    "aliases",
];

/// SQL keywords that signal a SQL value (case-insensitive prefix check).
const SQL_KEYWORDS: &[&str] = &[
    "SELECT", "INSERT", "UPDATE", "DELETE", "CREATE", "DROP", "ALTER", "WITH",
];

/// Heuristically infer the syntax from a value.
fn infer_syntax(value: &AliasValue) -> AliasSyntax {
    match value {
        AliasValue::Multiple(_) => AliasSyntax::Glob,
        AliasValue::Single(s) => {
            let trimmed = s.trim();
            // Check for SQL keywords
            let upper = trimmed.to_uppercase();
            if SQL_KEYWORDS.iter().any(|kw| upper.starts_with(kw)) {
                return AliasSyntax::Sql;
            }
            // Check if first token is a known subcommand
            if let Some(first_word) = trimmed.split_whitespace().next()
                && KNOWN_SUBCOMMANDS.contains(&first_word)
            {
                return AliasSyntax::Command;
            }
            // Check for glob metacharacters
            if trimmed.contains('*') || trimmed.contains('?') || trimmed.contains('[') {
                return AliasSyntax::Glob;
            }
            // Default to path
            AliasSyntax::Path
        }
    }
}

// ============================================================================
// AliasConfig
// ============================================================================

/// Unified alias configuration.
///
/// Handles both the new typed format and the legacy format for backward
/// compatibility:
///
/// New format:
/// ```toml
/// [aliases.vocabulary]
/// syntax = "command"
/// value = 'structure query "SELECT ..."'
/// description = "Most common words"
/// ```
///
/// Legacy format (treated as `syntax = "glob"`):
/// ```toml
/// [aliases]
/// tests = ["**/*test*"]
/// ```
#[derive(Debug, Clone, Default, Serialize)]
#[cfg_attr(feature = "config", derive(schemars::JsonSchema))]
pub struct AliasConfig {
    /// Map of alias names to their definitions.
    #[serde(flatten)]
    pub entries: HashMap<String, AliasEntry>,
}

// Custom Deserialize for backward compatibility with legacy `name = [...]` format.
impl<'de> Deserialize<'de> for AliasConfig {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        /// Handles typed entries, legacy glob arrays, and bare strings.
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum RawEntry {
            /// New format: `{ syntax = "...", value = "..." }`
            Typed(AliasEntry),
            /// Legacy format: `name = ["glob1", "glob2"]`
            Legacy(Vec<String>),
            /// Bare string: `name = "pattern"` — syntax inferred
            BareString(String),
        }

        let raw: HashMap<String, RawEntry> = HashMap::deserialize(deserializer)?;
        let entries = raw
            .into_iter()
            .map(|(name, raw)| {
                let entry = match raw {
                    RawEntry::Typed(e) => e,
                    RawEntry::Legacy(patterns) => AliasEntry {
                        syntax: Some(AliasSyntax::Glob),
                        value: AliasValue::Multiple(patterns),
                        description: None,
                    },
                    RawEntry::BareString(s) => AliasEntry {
                        syntax: None, // will be inferred + warned
                        value: AliasValue::Single(s),
                        description: None,
                    },
                };
                (name, entry)
            })
            .collect();
        Ok(AliasConfig { entries })
    }
}

impl AliasConfig {
    /// Names of all built-in aliases.
    pub fn builtin_names() -> &'static [&'static str] {
        &[
            "tests",
            "config",
            "build",
            "docs",
            "generated",
            "vocabulary",
            "stable-core",
            "unstable-core",
        ]
    }

    /// Get glob/path patterns for an alias, falling back to builtins.
    /// Returns None if the alias is unknown, disabled, or has non-filter syntax
    /// (command / sql).
    ///
    /// For language-aware builtins like `@tests`, pass detected languages.
    pub fn get(&self, name: &str) -> Option<Vec<String>> {
        self.get_with_languages(name, &[])
    }

    /// Get glob/path patterns for an alias with language context.
    pub fn get_with_languages(&self, name: &str, languages: &[&str]) -> Option<Vec<String>> {
        // Check user config first
        if let Some(entry) = self.entries.get(name) {
            if entry.value.is_empty() {
                return None; // Disabled
            }
            return match entry.resolved_syntax() {
                AliasSyntax::Glob | AliasSyntax::Path => Some(entry.value.as_strings()),
                AliasSyntax::Command | AliasSyntax::Sql => None,
            };
        }

        // Fall back to builtins
        let builtin = Self::builtin(name, languages)?;
        match builtin.resolved_syntax() {
            AliasSyntax::Glob | AliasSyntax::Path => Some(builtin.value.as_strings()),
            AliasSyntax::Command | AliasSyntax::Sql => None,
        }
    }

    /// Get the command string for a command-syntax alias.
    /// Returns None if the alias is unknown, disabled, or not a command alias.
    pub fn get_command(&self, name: &str) -> Option<String> {
        if let Some(entry) = self.entries.get(name) {
            if entry.value.is_empty() {
                return None;
            }
            if entry.resolved_syntax() == AliasSyntax::Command {
                return match &entry.value {
                    AliasValue::Single(s) => Some(s.clone()),
                    AliasValue::Multiple(v) => Some(v.join(" ")),
                };
            }
            return None;
        }
        let builtin = Self::builtin(name, &[])?;
        if builtin.resolved_syntax() == AliasSyntax::Command {
            match &builtin.value {
                AliasValue::Single(s) => Some(s.clone()),
                AliasValue::Multiple(v) => Some(v.join(" ")),
            }
        } else {
            None
        }
    }

    /// Get the full alias entry, resolved with builtins.
    /// Returns None if the alias is unknown or disabled.
    pub fn get_entry_resolved(&self, name: &str, languages: &[&str]) -> Option<AliasEntry> {
        if let Some(entry) = self.entries.get(name) {
            if entry.value.is_empty() {
                return None;
            }
            return Some(entry.clone());
        }
        Self::builtin(name, languages)
    }

    /// Get the syntax of an alias (user-defined or built-in).
    pub fn syntax_of(&self, name: &str) -> Option<AliasSyntax> {
        if let Some(entry) = self.entries.get(name) {
            return Some(entry.resolved_syntax());
        }
        Self::builtin(name, &[]).map(|e| e.resolved_syntax())
    }

    /// Built-in alias definitions.
    fn builtin(name: &str, languages: &[&str]) -> Option<AliasEntry> {
        match name {
            "tests" => {
                let mut patterns: Vec<String> = vec![];
                for lang in languages {
                    patterns.extend(normalize_language_meta::test_file_globs_for_language(lang));
                }
                patterns.sort_unstable();
                patterns.dedup();
                Some(AliasEntry {
                    syntax: Some(AliasSyntax::Glob),
                    value: AliasValue::Multiple(patterns),
                    description: Some("Test files".to_string()),
                })
            }
            "config" => Some(AliasEntry {
                syntax: Some(AliasSyntax::Glob),
                value: AliasValue::Multiple(
                    vec![
                        "*.toml",
                        "*.yaml",
                        "*.yml",
                        "*.json",
                        "*.ini",
                        "*.cfg",
                        ".env",
                        ".env.*",
                        "*.config.js",
                        "*.config.ts",
                    ]
                    .into_iter()
                    .map(String::from)
                    .collect(),
                ),
                description: Some("Configuration files".to_string()),
            }),
            "build" => Some(AliasEntry {
                syntax: Some(AliasSyntax::Glob),
                value: AliasValue::Multiple(
                    vec![
                        "target/**",
                        "dist/**",
                        "build/**",
                        "out/**",
                        "node_modules/**",
                        ".next/**",
                        ".nuxt/**",
                        "__pycache__/**",
                        "*.pyc",
                    ]
                    .into_iter()
                    .map(String::from)
                    .collect(),
                ),
                description: Some("Build artifacts and output directories".to_string()),
            }),
            "docs" => Some(AliasEntry {
                syntax: Some(AliasSyntax::Glob),
                value: AliasValue::Multiple(
                    vec![
                        "*.md",
                        "*.rst",
                        "*.txt",
                        "docs/**",
                        "doc/**",
                        "README*",
                        "CHANGELOG*",
                        "LICENSE*",
                    ]
                    .into_iter()
                    .map(String::from)
                    .collect(),
                ),
                description: Some("Documentation files".to_string()),
            }),
            "generated" => Some(AliasEntry {
                syntax: Some(AliasSyntax::Glob),
                value: AliasValue::Multiple(
                    vec![
                        "*.gen.*",
                        "*.generated.*",
                        "*.pb.go",
                        "*.pb.rs",
                        "*_generated.go",
                        "*_generated.rs",
                        "generated/**",
                    ]
                    .into_iter()
                    .map(String::from)
                    .collect(),
                ),
                description: Some("Generated code files".to_string()),
            }),
            "vocabulary" => Some(AliasEntry {
                syntax: Some(AliasSyntax::Command),
                value: AliasValue::Single(
                    "structure query \"SELECT word, COUNT(*) as count FROM symbol_words GROUP BY word ORDER BY count DESC\"".to_string()
                ),
                description: Some("Most common words in symbol names".to_string()),
            }),
            "stable-core" => Some(AliasEntry {
                syntax: Some(AliasSyntax::Command),
                value: AliasValue::Single(
                    "structure query \"SELECT f.file, f.commit_count, f.last_changed, COUNT(i.file) as fan_in FROM file_churn f JOIN imports i ON i.resolved_file = f.file GROUP BY f.file HAVING fan_in > 5 ORDER BY f.commit_count ASC, fan_in DESC\"".to_string()
                ),
                description: Some("Files with high fan-in and low churn".to_string()),
            }),
            "unstable-core" => Some(AliasEntry {
                syntax: Some(AliasSyntax::Command),
                value: AliasValue::Single(
                    "structure query \"SELECT f.file, f.commit_count, f.last_changed, COUNT(i.file) as fan_in FROM file_churn f JOIN imports i ON i.resolved_file = f.file GROUP BY f.file HAVING fan_in > 5 ORDER BY f.commit_count DESC, fan_in DESC\"".to_string()
                ),
                description: Some("Files with high fan-in and high churn".to_string()),
            }),
            _ => None,
        }
    }
}

// ============================================================================
// FilterError
// ============================================================================

/// Error returned by [`Filter::new`].
#[derive(Debug, thiserror::Error)]
pub enum FilterError {
    /// The pattern is not a valid glob.
    #[error("invalid filter pattern '{pattern}': {reason}")]
    InvalidPattern { pattern: String, reason: String },
    /// A bare word that looks like a language name was used instead of a glob or alias.
    #[error("{0}")]
    InvalidPatternHint(String),
    /// An `@alias` name is not defined.
    #[error("unknown alias @{0}")]
    UnknownAlias(String),
    /// An alias exists but has the wrong syntax for use as a filter pattern.
    #[error(
        "alias @{name} has syntax '{syntax}' and cannot be used as a filter pattern (only glob/path aliases work with --exclude/--only)"
    )]
    WrongSyntax { name: String, syntax: String },
}

impl From<FilterError> for String {
    fn from(e: FilterError) -> String {
        e.to_string()
    }
}

// ============================================================================
// Filter
// ============================================================================

/// Status of an alias (for display purposes).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[cfg_attr(feature = "config", derive(schemars::JsonSchema))]
#[serde(rename_all = "lowercase")]
pub enum AliasStatus {
    /// Built-in alias, unmodified
    Builtin,
    /// Custom alias defined in config
    Custom,
    /// Built-in alias disabled via empty value in config
    Disabled,
    /// Built-in alias overridden with new definition in config
    Overridden,
}

impl std::fmt::Display for AliasStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AliasStatus::Builtin => write!(f, "builtin"),
            AliasStatus::Custom => write!(f, "custom"),
            AliasStatus::Disabled => write!(f, "disabled"),
            AliasStatus::Overridden => write!(f, "overridden"),
        }
    }
}

/// Resolved alias information for display.
#[derive(Debug, Clone)]
pub struct ResolvedAlias {
    pub name: String,
    pub syntax: AliasSyntax,
    pub value: AliasValue,
    pub description: Option<String>,
    pub status: AliasStatus,
}

/// Result of resolving a filter value.
#[derive(Debug)]
pub enum AliasResolution {
    /// Resolved to glob patterns
    Patterns(Vec<String>),
    /// Alias not found
    UnknownAlias(String),
    /// Alias is disabled (empty value)
    DisabledAlias(String),
    /// Alias exists but has wrong syntax for filter use
    WrongSyntax { name: String, syntax: AliasSyntax },
}

/// Filter engine that resolves aliases and matches paths.
#[derive(Debug)]
pub struct Filter {
    /// Compiled exclude patterns
    exclude_matcher: Option<Gitignore>,
    /// Compiled include patterns (only mode)
    only_matcher: Option<Gitignore>,
    /// Warnings accumulated during construction
    warnings: Vec<String>,
}

impl Filter {
    /// Create a new filter from exclude/only patterns.
    ///
    /// Patterns starting with `@` are resolved as aliases (glob/path syntax only).
    /// Returns warnings for disabled aliases.
    pub fn new(
        exclude: &[String],
        only: &[String],
        config: &AliasConfig,
        languages: &[&str],
    ) -> Result<Self, FilterError> {
        let mut warnings = Vec::new();

        let exclude_matcher = if exclude.is_empty() {
            None
        } else {
            let patterns = resolve_patterns(exclude, config, languages, &mut warnings)?;
            if patterns.is_empty() {
                None
            } else {
                Some(build_matcher(&patterns)?)
            }
        };

        let only_matcher = if only.is_empty() {
            None
        } else {
            let patterns = resolve_patterns(only, config, languages, &mut warnings)?;
            if patterns.is_empty() {
                None
            } else {
                Some(build_matcher(&patterns)?)
            }
        };

        Ok(Self {
            exclude_matcher,
            only_matcher,
            warnings,
        })
    }

    /// Get warnings from filter construction.
    pub fn warnings(&self) -> &[String] {
        &self.warnings
    }

    /// Check if a path should be included.
    pub fn matches(&self, path: &Path) -> bool {
        if let Some(ref only) = self.only_matcher
            && !only.matched(path, false).is_ignore()
        {
            return false;
        }
        if let Some(ref exclude) = self.exclude_matcher
            && exclude.matched(path, false).is_ignore()
        {
            return false;
        }
        true
    }

    /// Check if any filters are active.
    #[allow(dead_code)]
    pub fn is_active(&self) -> bool {
        self.exclude_matcher.is_some() || self.only_matcher.is_some()
    }
}

/// Resolve patterns, expanding aliases.
fn resolve_patterns(
    patterns: &[String],
    config: &AliasConfig,
    languages: &[&str],
    warnings: &mut Vec<String>,
) -> Result<Vec<String>, FilterError> {
    let mut result = Vec::new();

    for pattern in patterns {
        if let Some(alias_name) = pattern.strip_prefix('@') {
            match resolve_alias(alias_name, config, languages) {
                AliasResolution::Patterns(ps) => {
                    result.extend(ps);
                }
                AliasResolution::UnknownAlias(name) => {
                    return Err(FilterError::UnknownAlias(name));
                }
                AliasResolution::DisabledAlias(name) => {
                    warnings.push(format!("@{} is disabled (matches nothing)", name));
                }
                AliasResolution::WrongSyntax { name, syntax } => {
                    return Err(FilterError::WrongSyntax {
                        name,
                        syntax: syntax.to_string(),
                    });
                }
            }
        } else if looks_like_language_name(pattern) {
            let matched_lang = languages
                .iter()
                .find(|l| l.eq_ignore_ascii_case(pattern))
                .copied();
            if let Some(lang) = matched_lang {
                return Err(FilterError::InvalidPatternHint(format!(
                    "'{pattern}' is not a valid pattern — use a glob like '*.ext' or an alias like '@tests' (run 'normalize aliases' to list available aliases; detected language: {lang})"
                )));
            } else {
                return Err(FilterError::InvalidPatternHint(format!(
                    "'{pattern}' is not a valid pattern — use a glob like '*.rs' or an alias like '@tests' (run 'normalize aliases' to list available aliases)"
                )));
            }
        } else {
            result.push(pattern.clone());
        }
    }

    Ok(result)
}

/// Returns true if `pattern` looks like a bare language name rather than a glob.
fn looks_like_language_name(pattern: &str) -> bool {
    !pattern.is_empty()
        && !pattern.contains(['*', '?', '{', '[', '/', '.'])
        && pattern
            .chars()
            .all(|c| c.is_alphabetic() || c == '-' || c == '_')
}

/// Resolve a single alias name for filter use (glob/path patterns only).
fn resolve_alias(name: &str, config: &AliasConfig, languages: &[&str]) -> AliasResolution {
    // Check if explicitly disabled
    if let Some(entry) = config.entries.get(name)
        && entry.value.is_empty()
    {
        return AliasResolution::DisabledAlias(name.to_string());
    }

    // Check if it exists but has wrong syntax for filter use
    if let Some(syntax) = config.syntax_of(name)
        && !matches!(syntax, AliasSyntax::Glob | AliasSyntax::Path)
    {
        return AliasResolution::WrongSyntax {
            name: name.to_string(),
            syntax,
        };
    }

    match config.get_with_languages(name, languages) {
        Some(patterns) => AliasResolution::Patterns(patterns),
        None => AliasResolution::UnknownAlias(name.to_string()),
    }
}

/// Build a gitignore-style matcher from patterns.
fn build_matcher(patterns: &[String]) -> Result<Gitignore, FilterError> {
    let mut builder = GitignoreBuilder::new("");

    for pattern in patterns {
        builder
            .add_line(None, pattern)
            .map_err(|e| FilterError::InvalidPattern {
                pattern: pattern.clone(),
                reason: e.to_string(),
            })?;
    }

    builder.build().map_err(|e| FilterError::InvalidPattern {
        pattern: String::new(),
        reason: e.to_string(),
    })
}

/// Get all resolved aliases for display.
pub fn list_aliases(config: &AliasConfig, languages: &[&str]) -> Vec<ResolvedAlias> {
    let mut aliases = Vec::new();
    let builtin_names = AliasConfig::builtin_names();

    for &name in builtin_names {
        if let Some(user_entry) = config.entries.get(name) {
            if user_entry.value.is_empty() {
                aliases.push(ResolvedAlias {
                    name: name.to_string(),
                    syntax: user_entry.resolved_syntax(),
                    value: AliasValue::Multiple(vec![]),
                    description: user_entry.description.clone(),
                    status: AliasStatus::Disabled,
                });
            } else {
                aliases.push(ResolvedAlias {
                    name: name.to_string(),
                    syntax: user_entry.resolved_syntax(),
                    value: user_entry.value.clone(),
                    description: user_entry.description.clone(),
                    status: AliasStatus::Overridden,
                });
            }
        } else if let Some(builtin) = AliasConfig::builtin(name, languages) {
            aliases.push(ResolvedAlias {
                name: name.to_string(),
                syntax: builtin.resolved_syntax(),
                value: builtin.value,
                description: builtin.description,
                status: AliasStatus::Builtin,
            });
        }
    }

    let builtin_set: std::collections::HashSet<&str> = builtin_names.iter().copied().collect();
    for (name, entry) in &config.entries {
        if !builtin_set.contains(name.as_str()) {
            aliases.push(ResolvedAlias {
                name: name.clone(),
                syntax: entry.resolved_syntax(),
                value: entry.value.clone(),
                description: entry.description.clone(),
                status: AliasStatus::Custom,
            });
        }
    }

    aliases.sort_by(|a, b| {
        let a_builtin = matches!(
            a.status,
            AliasStatus::Builtin | AliasStatus::Disabled | AliasStatus::Overridden
        );
        let b_builtin = matches!(
            b.status,
            AliasStatus::Builtin | AliasStatus::Disabled | AliasStatus::Overridden
        );
        match (a_builtin, b_builtin) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => a.name.cmp(&b.name),
        }
    });

    aliases
}

/// Validate alias entries at load time and emit warnings for problems.
///
/// Does not hard-error so a config typo does not break all of normalize.
pub fn validate_aliases(config: &AliasConfig) {
    for (name, entry) in &config.entries {
        if entry.syntax.is_none() {
            let inferred = entry.resolved_syntax();
            tracing::warn!(
                "alias @{}: missing 'syntax' field; inferred as '{}'. \
                 Add `syntax = \"{}\"` to silence this warning.",
                name,
                inferred,
                inferred,
            );
        }
        match entry.resolved_syntax() {
            AliasSyntax::Glob => {
                for pattern in entry.value.as_strings() {
                    let mut builder = GitignoreBuilder::new("");
                    if let Err(e) = builder.add_line(None, &pattern) {
                        tracing::warn!(
                            "alias @{}: invalid glob pattern '{}': {}",
                            name,
                            pattern,
                            e
                        );
                    }
                }
            }
            AliasSyntax::Command => {
                let cmd = match &entry.value {
                    AliasValue::Single(s) => s.clone(),
                    AliasValue::Multiple(v) => v.join(" "),
                };
                if cmd.is_empty() {
                    tracing::warn!("alias @{}: command value is empty", name);
                } else if let Err(e) = shell_words::split(&cmd) {
                    tracing::warn!(
                        "alias @{}: invalid shell syntax in command value: {}",
                        name,
                        e
                    );
                }
            }
            AliasSyntax::Path => {
                if entry.value.is_empty() {
                    tracing::warn!("alias @{}: path value is empty", name);
                }
            }
            AliasSyntax::Sql => {
                let sql = match &entry.value {
                    AliasValue::Single(s) => s.clone(),
                    AliasValue::Multiple(v) => v.join(" "),
                };
                if sql.is_empty() {
                    tracing::warn!("alias @{}: sql value is empty", name);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_glob_pattern() {
        let config = AliasConfig::default();
        let filter =
            // normalize-syntax-allow: rust/unwrap-in-impl - test code, panic is appropriate
            Filter::new(&["*.test.js".to_string()], &[], &config, &["javascript"]).unwrap();

        assert!(filter.is_active());
        assert!(!filter.matches(Path::new("foo.test.js")));
        assert!(filter.matches(Path::new("foo.js")));
    }

    #[test]
    fn test_resolve_alias() {
        let config = AliasConfig::default();
        // normalize-syntax-allow: rust/unwrap-in-impl - test code, panic is appropriate
        let filter = Filter::new(&["@tests".to_string()], &[], &config, &["go"]).unwrap();

        assert!(filter.is_active());
        assert!(!filter.matches(Path::new("foo_test.go")));
        assert!(filter.matches(Path::new("foo.go")));
    }

    #[test]
    fn test_unknown_alias_error() {
        let config = AliasConfig::default();
        let result = Filter::new(&["@unknown".to_string()], &[], &config, &[]);

        assert!(result.is_err());
        // normalize-syntax-allow: rust/unwrap-in-impl - test code, panic is appropriate
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("unknown alias @unknown")
        );
    }

    #[test]
    fn test_command_alias_in_filter_error() {
        let config = AliasConfig::default();
        let result = Filter::new(&["@vocabulary".to_string()], &[], &config, &[]);
        assert!(result.is_err());
        // normalize-syntax-allow: rust/unwrap-in-impl - test code, panic is appropriate
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("syntax 'command'"),
            "expected wrong-syntax error, got: {err}"
        );
    }

    #[test]
    fn test_disabled_alias_warning() {
        let mut config = AliasConfig::default();
        config.entries.insert(
            "tests".to_string(),
            AliasEntry {
                syntax: Some(AliasSyntax::Glob),
                value: AliasValue::Multiple(vec![]),
                description: None,
            },
        );

        // normalize-syntax-allow: rust/unwrap-in-impl - test code, panic is appropriate
        let filter = Filter::new(&["@tests".to_string()], &[], &config, &["Go"]).unwrap();

        assert!(!filter.is_active());
        assert_eq!(filter.warnings().len(), 1);
        assert!(filter.warnings()[0].contains("disabled"));
    }

    #[test]
    fn test_config_override() {
        let mut config = AliasConfig::default();
        config.entries.insert(
            "tests".to_string(),
            AliasEntry {
                syntax: Some(AliasSyntax::Glob),
                value: AliasValue::Multiple(vec!["my_tests/**".to_string()]),
                description: None,
            },
        );

        // normalize-syntax-allow: rust/unwrap-in-impl - test code, panic is appropriate
        let filter = Filter::new(&["@tests".to_string()], &[], &config, &["Go"]).unwrap();

        assert!(filter.is_active());
        assert!(!filter.matches(Path::new("my_tests/foo.go")));
        assert!(filter.matches(Path::new("foo_test.go")));
    }

    #[test]
    fn test_only_mode() {
        let config = AliasConfig::default();
        // normalize-syntax-allow: rust/unwrap-in-impl - test code, panic is appropriate
        let filter = Filter::new(&[], &["*.rs".to_string()], &config, &[]).unwrap();

        assert!(filter.is_active());
        assert!(filter.matches(Path::new("foo.rs")));
        assert!(!filter.matches(Path::new("foo.go")));
    }

    #[test]
    fn test_bare_language_name_error() {
        let config = AliasConfig::default();
        let result = Filter::new(&[], &["rust".to_string()], &config, &["Rust"]);
        assert!(result.is_err());
        // normalize-syntax-allow: rust/unwrap-in-impl - test code, panic is appropriate
        let err = result.unwrap_err().to_string();
        assert!(err.contains("'rust' is not a valid pattern"), "got: {err}");
        assert!(
            err.contains("Rust"),
            "should mention detected language, got: {err}"
        );
    }

    #[test]
    fn test_bare_language_name_no_detected_language() {
        let config = AliasConfig::default();
        let result = Filter::new(&[], &["python".to_string()], &config, &["Rust"]);
        assert!(result.is_err());
        // normalize-syntax-allow: rust/unwrap-in-impl - test code, panic is appropriate
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("'python' is not a valid pattern"),
            "got: {err}"
        );
    }

    #[test]
    fn test_list_aliases() {
        let mut config = AliasConfig::default();
        config.entries.insert(
            "tests".to_string(),
            AliasEntry {
                syntax: Some(AliasSyntax::Glob),
                value: AliasValue::Multiple(vec![]),
                description: None,
            },
        );
        config.entries.insert(
            "vendor".to_string(),
            AliasEntry {
                syntax: Some(AliasSyntax::Glob),
                value: AliasValue::Multiple(vec!["vendor/**".to_string()]),
                description: Some("Vendored code".to_string()),
            },
        );

        let aliases = list_aliases(&config, &["rust"]);

        // normalize-syntax-allow: rust/unwrap-in-impl - test code, panic is appropriate
        let tests = aliases.iter().find(|a| a.name == "tests").unwrap();
        assert_eq!(tests.status, AliasStatus::Disabled);

        // normalize-syntax-allow: rust/unwrap-in-impl - test code, panic is appropriate
        let vendor = aliases.iter().find(|a| a.name == "vendor").unwrap();
        assert_eq!(vendor.status, AliasStatus::Custom);
        assert_eq!(vendor.syntax, AliasSyntax::Glob);

        // normalize-syntax-allow: rust/unwrap-in-impl - test code, panic is appropriate
        let docs = aliases.iter().find(|a| a.name == "docs").unwrap();
        assert_eq!(docs.status, AliasStatus::Builtin);

        // normalize-syntax-allow: rust/unwrap-in-impl - test code, panic is appropriate
        let vocab = aliases.iter().find(|a| a.name == "vocabulary").unwrap();
        assert_eq!(vocab.status, AliasStatus::Builtin);
        assert_eq!(vocab.syntax, AliasSyntax::Command);
    }

    #[test]
    fn test_get_command() {
        let config = AliasConfig::default();
        assert!(config.get_command("vocabulary").is_some());
        // normalize-syntax-allow: rust/unwrap-in-impl - test code, panic is appropriate
        assert!(
            config
                .get_command("vocabulary")
                .unwrap()
                .contains("structure query")
        );
        assert!(config.get_command("tests").is_none());
        assert!(config.get_command("nonexistent").is_none());
    }

    #[test]
    fn test_legacy_format_compat() {
        let toml_str = r#"
tests = ["my_tests/**"]
vendor = ["vendor/**", "third_party/**"]
config = []
"#;
        // normalize-syntax-allow: rust/unwrap-in-impl - test code, panic is appropriate
        let config: AliasConfig = toml::from_str(toml_str).unwrap();

        // normalize-syntax-allow: rust/unwrap-in-impl - test code, panic is appropriate
        let tests = config.entries.get("tests").unwrap();
        assert_eq!(tests.syntax, Some(AliasSyntax::Glob));
        assert_eq!(
            tests.value,
            AliasValue::Multiple(vec!["my_tests/**".to_string()])
        );

        // normalize-syntax-allow: rust/unwrap-in-impl - test code, panic is appropriate
        let cfg = config.entries.get("config").unwrap();
        assert!(cfg.value.is_empty());
    }

    #[test]
    fn test_new_format() {
        let toml_str = r#"
[vocabulary]
syntax = "command"
value = 'structure query "SELECT 1"'
description = "Test command alias"

[my-filter]
syntax = "glob"
value = ["*.rs", "*.toml"]
"#;
        // normalize-syntax-allow: rust/unwrap-in-impl - test code, panic is appropriate
        let config: AliasConfig = toml::from_str(toml_str).unwrap();

        // normalize-syntax-allow: rust/unwrap-in-impl - test code, panic is appropriate
        let vocab = config.entries.get("vocabulary").unwrap();
        assert_eq!(vocab.syntax, Some(AliasSyntax::Command));
        assert_eq!(
            vocab.value,
            AliasValue::Single("structure query \"SELECT 1\"".to_string())
        );
        assert_eq!(vocab.description, Some("Test command alias".to_string()));

        // normalize-syntax-allow: rust/unwrap-in-impl - test code, panic is appropriate
        let filter = config.entries.get("my-filter").unwrap();
        assert_eq!(filter.syntax, Some(AliasSyntax::Glob));
        assert_eq!(
            filter.value,
            AliasValue::Multiple(vec!["*.rs".to_string(), "*.toml".to_string()])
        );
        assert!(filter.description.is_none());
    }

    #[test]
    fn test_bare_string_format() {
        let toml_str = r#"
my-tests = "**/*test*"
my-cmd = "view src/"
"#;
        // normalize-syntax-allow: rust/unwrap-in-impl - test code, panic is appropriate
        let config: AliasConfig = toml::from_str(toml_str).unwrap();

        let tests = config.entries.get("my-tests").unwrap();
        assert_eq!(tests.syntax, None); // syntax not declared
        assert_eq!(tests.resolved_syntax(), AliasSyntax::Glob); // inferred from glob chars
        assert_eq!(tests.value, AliasValue::Single("**/*test*".to_string()));

        let cmd = config.entries.get("my-cmd").unwrap();
        assert_eq!(cmd.syntax, None);
        assert_eq!(cmd.resolved_syntax(), AliasSyntax::Command); // "view" is a known subcommand
    }

    #[test]
    fn test_syntax_inference() {
        // SQL inference
        let sql_entry = AliasEntry {
            syntax: None,
            value: AliasValue::Single("SELECT * FROM symbols".to_string()),
            description: None,
        };
        assert_eq!(sql_entry.resolved_syntax(), AliasSyntax::Sql);

        // Command inference
        let cmd_entry = AliasEntry {
            syntax: None,
            value: AliasValue::Single("rank complexity src/".to_string()),
            description: None,
        };
        assert_eq!(cmd_entry.resolved_syntax(), AliasSyntax::Command);

        // Glob inference from metacharacters
        let glob_entry = AliasEntry {
            syntax: None,
            value: AliasValue::Single("src/**/*.rs".to_string()),
            description: None,
        };
        assert_eq!(glob_entry.resolved_syntax(), AliasSyntax::Glob);

        // Multiple values always infer as glob
        let multi_entry = AliasEntry {
            syntax: None,
            value: AliasValue::Multiple(vec!["a".to_string(), "b".to_string()]),
            description: None,
        };
        assert_eq!(multi_entry.resolved_syntax(), AliasSyntax::Glob);

        // Plain path (no metacharacters, no known subcommand, no SQL)
        let path_entry = AliasEntry {
            syntax: None,
            value: AliasValue::Single("src/lib.rs".to_string()),
            description: None,
        };
        assert_eq!(path_entry.resolved_syntax(), AliasSyntax::Path);

        // Explicit syntax is preserved
        let explicit = AliasEntry {
            syntax: Some(AliasSyntax::Path),
            value: AliasValue::Single("view src/".to_string()), // would infer Command, but explicit wins
            description: None,
        };
        assert_eq!(explicit.resolved_syntax(), AliasSyntax::Path);
    }
}
