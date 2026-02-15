//! Filter system for --exclude and --only flags.
//!
//! Supports:
//! - Glob patterns: `--exclude="*_test.go"`, `--only="*.rs"`
//! - Aliases: `--exclude=@tests`, `--only=@docs`
//!
//! Built-in aliases are language-aware (e.g., @tests includes `*_test.go` for Go,
//! `test_*.py` for Python). Config can override or add new aliases via `[aliases]`.

use ignore::gitignore::{Gitignore, GitignoreBuilder};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

// ============================================================================
// AliasConfig
// ============================================================================

/// Unified alias configuration for @ prefix expansion.
/// Used for both command targets (`normalize view @todo`) and filters (`--only @tests`).
///
/// Example:
/// ```toml
/// [aliases]
/// todo = ["TODO.md"]              # @todo → specific file
/// config = [".normalize/config.toml"]  # overrides built-in @config
/// vendor = ["vendor/**"]          # custom filter alias
/// tests = []                      # disable built-in @tests
/// ```
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[cfg_attr(
    feature = "config",
    derive(normalize_derive::Merge, schemars::JsonSchema)
)]
#[serde(default)]
pub struct AliasConfig {
    /// Map alias names to paths/patterns. Empty array disables the alias.
    #[serde(flatten)]
    pub entries: HashMap<String, Vec<String>>,
}

impl AliasConfig {
    /// Names of all built-in aliases.
    pub fn builtin_names() -> &'static [&'static str] {
        &["tests", "config", "build", "docs", "generated"]
    }

    /// Get values for an alias, falling back to builtins.
    /// Returns None if alias is unknown or disabled (empty array).
    ///
    /// For language-aware builtins like @tests, pass detected languages.
    pub fn get(&self, name: &str) -> Option<Vec<String>> {
        self.get_with_languages(name, &[])
    }

    /// Get values for an alias with language context for builtins like @tests.
    pub fn get_with_languages(&self, name: &str, languages: &[&str]) -> Option<Vec<String>> {
        // Check user config first
        if let Some(values) = self.entries.get(name) {
            if values.is_empty() {
                return None; // Disabled
            }
            return Some(values.clone());
        }

        // Fall back to builtins
        Self::builtin(name, languages)
    }

    /// Built-in alias patterns.
    fn builtin(name: &str, languages: &[&str]) -> Option<Vec<String>> {
        let patterns: Vec<&str> = match name {
            "tests" => {
                let mut p = vec!["**/test_*.py", "**/*_test.py", "**/tests/**"];
                for lang in languages {
                    match *lang {
                        "go" => p.extend(["*_test.go", "**/*_test.go"]),
                        "rust" => p.extend(["**/tests/**/*.rs"]),
                        "javascript" | "typescript" => p.extend([
                            "**/*.test.js",
                            "**/*.spec.js",
                            "**/*.test.ts",
                            "**/*.spec.ts",
                            "**/__tests__/**",
                        ]),
                        "java" => p.extend(["**/test/**", "**/*Test.java"]),
                        "ruby" => {
                            p.extend(["**/test/**", "**/*_test.rb", "**/spec/**", "**/*_spec.rb"])
                        }
                        _ => {}
                    }
                }
                p
            }
            "config" => vec![
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
            ],
            "build" => vec![
                "target/**",
                "dist/**",
                "build/**",
                "out/**",
                "node_modules/**",
                ".next/**",
                ".nuxt/**",
                "__pycache__/**",
                "*.pyc",
            ],
            "docs" => vec![
                "*.md",
                "*.rst",
                "*.txt",
                "docs/**",
                "doc/**",
                "README*",
                "CHANGELOG*",
                "LICENSE*",
            ],
            "generated" => vec![
                "*.gen.*",
                "*.generated.*",
                "*.pb.go",
                "*.pb.rs",
                "*_generated.go",
                "*_generated.rs",
                "generated/**",
            ],
            _ => return None,
        };
        Some(patterns.into_iter().map(String::from).collect())
    }
}

// ============================================================================
// Filter
// ============================================================================

/// Status of an alias (for display purposes).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AliasStatus {
    /// Built-in alias, unmodified
    Builtin,
    /// Custom alias defined in config
    Custom,
    /// Built-in alias disabled via empty array in config
    Disabled,
    /// Built-in alias overridden with new patterns in config
    Overridden,
}

/// Resolved alias information for display.
#[derive(Debug, Clone)]
pub struct ResolvedAlias {
    pub name: String,
    pub patterns: Vec<String>,
    pub status: AliasStatus,
}

/// Result of resolving a filter value.
#[derive(Debug)]
pub enum ResolveResult {
    /// Resolved to glob patterns
    Patterns(Vec<String>),
    /// Alias not found
    UnknownAlias(String),
    /// Alias is disabled (empty patterns)
    DisabledAlias(String),
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
    /// Patterns starting with `@` are resolved as aliases.
    /// Returns warnings for disabled aliases.
    pub fn new(
        exclude: &[String],
        only: &[String],
        config: &AliasConfig,
        languages: &[&str],
    ) -> Result<Self, String> {
        let mut warnings = Vec::new();

        // Build exclude matcher
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

        // Build only matcher
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
    ///
    /// Returns true if the path passes the filter.
    pub fn matches(&self, path: &Path) -> bool {
        // If only matcher exists, path must match it
        if let Some(ref only) = self.only_matcher
            && !only.matched(path, false).is_ignore()
        {
            return false;
        }

        // If exclude matcher exists, path must not match it
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
) -> Result<Vec<String>, String> {
    let mut result = Vec::new();

    for pattern in patterns {
        if let Some(alias_name) = pattern.strip_prefix('@') {
            match resolve_alias(alias_name, config, languages) {
                ResolveResult::Patterns(ps) => {
                    result.extend(ps);
                }
                ResolveResult::UnknownAlias(name) => {
                    return Err(format!("unknown alias @{}", name));
                }
                ResolveResult::DisabledAlias(name) => {
                    warnings.push(format!("@{} is disabled (matches nothing)", name));
                }
            }
        } else {
            result.push(pattern.clone());
        }
    }

    Ok(result)
}

/// Resolve a single alias name to patterns.
fn resolve_alias(name: &str, config: &AliasConfig, languages: &[&str]) -> ResolveResult {
    // Check if explicitly disabled
    if let Some(patterns) = config.entries.get(name)
        && patterns.is_empty()
    {
        return ResolveResult::DisabledAlias(name.to_string());
    }

    // Use unified alias lookup
    match config.get_with_languages(name, languages) {
        Some(patterns) => ResolveResult::Patterns(patterns),
        None => ResolveResult::UnknownAlias(name.to_string()),
    }
}

/// Build a gitignore-style matcher from patterns.
fn build_matcher(patterns: &[String]) -> Result<Gitignore, String> {
    let mut builder = GitignoreBuilder::new("");

    for pattern in patterns {
        builder
            .add_line(None, pattern)
            .map_err(|e| format!("invalid glob pattern '{}': {}", pattern, e))?;
    }

    builder
        .build()
        .map_err(|e| format!("failed to build filter: {}", e))
}

/// Get all resolved aliases for display (normalize filter aliases).
pub fn list_aliases(config: &AliasConfig, languages: &[&str]) -> Vec<ResolvedAlias> {
    let mut aliases = Vec::new();
    let builtin_names = AliasConfig::builtin_names();

    // Process built-in aliases
    for &name in builtin_names {
        if let Some(user_patterns) = config.entries.get(name) {
            if user_patterns.is_empty() {
                aliases.push(ResolvedAlias {
                    name: name.to_string(),
                    patterns: vec![],
                    status: AliasStatus::Disabled,
                });
            } else {
                aliases.push(ResolvedAlias {
                    name: name.to_string(),
                    patterns: user_patterns.clone(),
                    status: AliasStatus::Overridden,
                });
            }
        } else if let Some(patterns) = config.get_with_languages(name, languages) {
            aliases.push(ResolvedAlias {
                name: name.to_string(),
                patterns,
                status: AliasStatus::Builtin,
            });
        }
    }

    // Add custom aliases from config
    let builtin_set: std::collections::HashSet<&str> = builtin_names.iter().copied().collect();
    for (name, patterns) in &config.entries {
        if !builtin_set.contains(name.as_str()) {
            aliases.push(ResolvedAlias {
                name: name.clone(),
                patterns: patterns.clone(),
                status: AliasStatus::Custom,
            });
        }
    }

    // Sort: built-ins first, then custom
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_glob_pattern() {
        let config = AliasConfig::default();
        let filter =
            Filter::new(&["*.test.js".to_string()], &[], &config, &["javascript"]).unwrap();

        assert!(filter.is_active());
        assert!(!filter.matches(Path::new("foo.test.js")));
        assert!(filter.matches(Path::new("foo.js")));
    }

    #[test]
    fn test_resolve_alias() {
        let config = AliasConfig::default();
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
        assert!(result.unwrap_err().contains("unknown alias @unknown"));
    }

    #[test]
    fn test_disabled_alias_warning() {
        let mut config = AliasConfig::default();
        config.entries.insert("tests".to_string(), vec![]);

        let filter = Filter::new(&["@tests".to_string()], &[], &config, &["go"]).unwrap();

        assert!(!filter.is_active()); // No patterns = not active
        assert_eq!(filter.warnings().len(), 1);
        assert!(filter.warnings()[0].contains("disabled"));
    }

    #[test]
    fn test_config_override() {
        let mut config = AliasConfig::default();
        config
            .entries
            .insert("tests".to_string(), vec!["my_tests/**".to_string()]);

        let filter = Filter::new(&["@tests".to_string()], &[], &config, &["go"]).unwrap();

        assert!(filter.is_active());
        assert!(!filter.matches(Path::new("my_tests/foo.go")));
        assert!(filter.matches(Path::new("foo_test.go"))); // Built-in pattern not applied
    }

    #[test]
    fn test_only_mode() {
        let config = AliasConfig::default();
        let filter = Filter::new(&[], &["*.rs".to_string()], &config, &[]).unwrap();

        assert!(filter.is_active());
        assert!(filter.matches(Path::new("foo.rs")));
        assert!(!filter.matches(Path::new("foo.go")));
    }

    #[test]
    fn test_list_aliases() {
        let mut config = AliasConfig::default();
        config.entries.insert("tests".to_string(), vec![]); // Disabled
        config
            .entries
            .insert("vendor".to_string(), vec!["vendor/**".to_string()]); // Custom

        let aliases = list_aliases(&config, &["rust"]);

        let tests = aliases.iter().find(|a| a.name == "tests").unwrap();
        assert_eq!(tests.status, AliasStatus::Disabled);

        let vendor = aliases.iter().find(|a| a.name == "vendor").unwrap();
        assert_eq!(vendor.status, AliasStatus::Custom);

        let docs = aliases.iter().find(|a| a.name == "docs").unwrap();
        assert_eq!(docs.status, AliasStatus::Builtin);
    }
}
