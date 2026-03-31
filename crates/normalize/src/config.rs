//! Configuration system for normalize.
//!
//! Loads config from:
//! 1. Global: ~/.config/normalize/config.toml
//! 2. Per-project: .normalize/config.toml (overrides global)
//!
//! Example config.toml:
//! ```toml
//! [daemon]
//! enabled = true
//! auto_start = true
//!
//! [index]
//! enabled = true
//!
//! [shadow]
//! enabled = true                # auto-track edits for undo/redo
//! warn_on_delete = true         # confirm before deleting symbols
//!
//! [aliases]
//! todo = ["TODO.md", "TASKS.md"]   # @todo for command targets AND filters
//! config = [".normalize/config.toml"]   # overrides built-in @config
//! vendor = ["vendor/**"]           # custom alias for filters
//! tests = []                       # disable built-in @tests
//!
//! [todo]
//! file = "TASKS.md"           # custom todo file (default: auto-detect)
//! primary_section = "Backlog" # default section for add/done/rm
//! show_all = true             # show all sections by default
//!
//! [view]
//! depth = 2                   # default tree depth (0=names, 1=signatures, 2=children)
//! line_numbers = true         # show line numbers by default
//! show_docs = true            # show full docstrings by default
//! context_files = ["README.md", "SUMMARY.md", ".context.md"]  # preamble files for directory views
//!
//! [analyze]
//! threshold = 10              # only show functions with complexity >= 10
//! compact = true              # use compact output for --overview
//!
//! [analyze.duplicates]
//! min_lines = 15              # minimum lines for duplicate detection
//!
//! [rules]
//! global-allow = ["**/tests/fixtures/**"]
//!
//! [rules."rust/unwrap-in-impl"]
//! severity = "error"
//! allow = ["crates/*/src/lib.rs"]
//!
//! [[rules.sarif-tools]]
//! name = "eslint"
//! command = ["npx", "eslint", "--format", "json", "{root}"]
//!
//! [text-search]
//! limit = 50                  # default max results
//! ignore_case = true          # case-insensitive by default
//!
//! [pretty]
//! enabled = true              # auto-enable when TTY (default: auto)
//! colors = "auto"             # "auto", "always", or "never"
//! highlight = true            # syntax highlighting on signatures
//!
//! [serve]
//! fact_debounce_ms = 1500     # debounce for LSP fact diagnostics (ms)
//! ```

use crate::commands::analyze::AnalyzeConfig;
use crate::commands::text_search::TextSearchConfig;
use crate::commands::view::ViewConfig;
use crate::daemon::DaemonConfig;
use crate::filter::AliasConfig;
use crate::output::PrettyConfig;
use crate::shadow::ShadowConfig;
use normalize_budget::BudgetConfig;
use normalize_ratchet::RatchetConfig;
use normalize_rules::RulesConfig;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Index configuration.
#[derive(Debug, Clone, Deserialize, Serialize, Default, JsonSchema, server_less::Config)]
#[serde(default)]
pub struct IndexConfig {
    /// Whether to create and use the file index. Default: true
    pub enabled: Option<bool>,
}

impl IndexConfig {
    pub fn enabled(&self) -> bool {
        self.enabled.unwrap_or(true)
    }
}

/// User-defined rule tag groups.
///
/// Tags can reference rule IDs or other tag names (including built-in tags).
/// References are resolved transitively at filter time.
///
/// Example:
/// ```toml
/// [rule-tags]
/// ci-blockers = ["security", "error-handling"]   # group of tags
/// my-checks   = ["circular-deps", "hub-file"]    # group of rule IDs
/// strict      = ["ci-blockers", "my-checks"]     # references other user tags
/// ```
#[derive(Debug, Clone, Deserialize, Serialize, Default, JsonSchema)]
#[serde(transparent)]
pub struct RuleTagsConfig(pub std::collections::HashMap<String, Vec<String>>);

/// Root configuration structure.
#[derive(Debug, Clone, Deserialize, Serialize, Default, JsonSchema, server_less::Config)]
#[serde(default)]
pub struct NormalizeConfig {
    #[param(nested)]
    pub daemon: DaemonConfig,
    #[param(nested)]
    pub index: IndexConfig,
    #[param(nested, serde)]
    pub shadow: ShadowConfig,
    #[param(nested, serde)]
    pub aliases: AliasConfig,
    #[param(nested, serde)]
    pub view: ViewConfig,
    #[param(nested, serde)]
    pub analyze: AnalyzeConfig,
    /// Rules configuration: per-rule overrides, global-allow, and sarif-tools.
    /// Configured via `[rules]`, `[rules."rule-id"]`, and `[[rules.sarif-tools]]`.
    #[param(nested, serde)]
    pub rules: RulesConfig,
    #[serde(rename = "text-search")]
    #[param(nested, file_key = "text-search")]
    pub text_search: TextSearchConfig,
    #[param(nested, serde)]
    pub pretty: PrettyConfig,
    #[param(nested)]
    pub serve: crate::serve::ServeConfig,
    /// User-defined rule tag groups (`[rule-tags]` section).
    #[serde(default, rename = "rule-tags")]
    #[param(nested, serde, file_key = "rule-tags")]
    pub rule_tags: RuleTagsConfig,
    /// Ratchet metric regression tracking (`[ratchet]` section).
    #[param(nested, serde)]
    pub ratchet: RatchetConfig,
    /// Diff-based budget tracking (`[budget]` section).
    #[param(nested, serde)]
    pub budget: BudgetConfig,
    /// Semantic embeddings configuration (`[embeddings]` section).
    #[param(nested, serde)]
    pub embeddings: normalize_semantic::EmbeddingsConfig,
}

impl NormalizeConfig {
    /// Load configuration for a project.
    ///
    /// Loads global config from ~/.config/normalize/config.toml,
    /// then per-project config from .normalize/config.toml (overrides global).
    pub fn load(root: &Path) -> Self {
        let mut sources = vec![server_less::ConfigSource::Defaults];
        if let Some(global_path) = Self::global_config_path() {
            sources.push(server_less::ConfigSource::File(global_path));
        }
        let project_config = root.join(".normalize").join("config.toml");
        sources.push(server_less::ConfigSource::File(project_config.clone()));
        <Self as server_less::ConfigTrait>::load(&sources).unwrap_or_else(|e| {
            // Warn on parse errors so the user knows their config is being ignored.
            // Missing files are silently skipped by ConfigTrait; only real errors surface here.
            eprintln!(
                "warning: failed to load {}: {} (using defaults — run `normalize config validate` for details)",
                project_config.display(),
                e
            );
            Self::default()
        })
    }

    /// Get the global config path.
    pub fn global_config_path() -> Option<std::path::PathBuf> {
        let config_home = std::env::var("XDG_CONFIG_HOME")
            .map(std::path::PathBuf::from)
            .ok()
            .or_else(|| dirs::home_dir().map(|h| h.join(".config")))?;
        Some(config_home.join("normalize").join("config.toml"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    #[test]
    fn test_default_config() {
        let config = NormalizeConfig::default();
        assert!(config.daemon.enabled());
        assert!(config.daemon.auto_start());
        assert!(config.index.enabled());
    }

    #[test]
    fn test_load_project_config() {
        let dir = TempDir::new().unwrap();
        let moss_dir = dir.path().join(".normalize");
        std::fs::create_dir_all(&moss_dir).unwrap();

        let config_path = moss_dir.join("config.toml");
        let mut file = std::fs::File::create(&config_path).unwrap();
        writeln!(
            file,
            r#"
[daemon]
enabled = false
auto_start = false

[index]
enabled = true
"#
        )
        .unwrap();

        let config = NormalizeConfig::load(dir.path());
        assert!(!config.daemon.enabled());
        assert!(!config.daemon.auto_start());
        assert!(config.index.enabled());
    }

    #[test]
    fn test_partial_config() {
        let dir = TempDir::new().unwrap();
        let moss_dir = dir.path().join(".normalize");
        std::fs::create_dir_all(&moss_dir).unwrap();

        let config_path = moss_dir.join("config.toml");
        let mut file = std::fs::File::create(&config_path).unwrap();
        writeln!(
            file,
            r#"
[daemon]
auto_start = false
"#
        )
        .unwrap();

        let config = NormalizeConfig::load(dir.path());
        // enabled is None (not specified), accessor returns true
        assert!(config.daemon.enabled());
        assert!(!config.daemon.auto_start());
    }

    #[test]
    fn test_aliases_config() {
        let dir = TempDir::new().unwrap();
        let moss_dir = dir.path().join(".normalize");
        std::fs::create_dir_all(&moss_dir).unwrap();

        let config_path = moss_dir.join("config.toml");
        let mut file = std::fs::File::create(&config_path).unwrap();
        writeln!(
            file,
            r#"
[aliases]
tests = ["my_tests/**"]
vendor = ["vendor/**", "third_party/**"]
config = []
"#
        )
        .unwrap();

        let config = NormalizeConfig::load(dir.path());
        assert_eq!(
            config.aliases.entries.get("tests"),
            Some(&vec!["my_tests/**".to_string()])
        );
        assert_eq!(
            config.aliases.entries.get("vendor"),
            Some(&vec!["vendor/**".to_string(), "third_party/**".to_string()])
        );
        // Empty array disables alias
        assert_eq!(config.aliases.entries.get("config"), Some(&vec![]));
    }

    #[test]
    fn test_global_project_layering() {
        // Global sets enabled=false; project only sets auto_start=true.
        // Config::load(File(global), File(project)) must preserve enabled=false.
        use server_less::{ConfigSource, ConfigTrait};

        let tmp = TempDir::new().unwrap();
        let global = tmp.path().join("global.toml");
        let project_dir = tmp.path().join("project");
        std::fs::create_dir_all(project_dir.join(".normalize")).unwrap();
        let project = project_dir.join(".normalize").join("config.toml");

        std::fs::write(&global, "[daemon]\nenabled = false\n").unwrap();
        std::fs::write(&project, "[daemon]\nauto_start = true\n").unwrap();

        let config = <NormalizeConfig as ConfigTrait>::load(&[
            ConfigSource::Defaults,
            ConfigSource::File(global),
            ConfigSource::File(project),
        ])
        .unwrap_or_default();

        // enabled=false must come from global, not be reset by project's absent field
        assert!(!config.daemon.enabled());
        // auto_start=true must come from project
        assert!(config.daemon.auto_start());
    }

    #[test]
    fn test_pretty_config() {
        use crate::output::ColorMode;

        let dir = TempDir::new().unwrap();
        let moss_dir = dir.path().join(".normalize");
        std::fs::create_dir_all(&moss_dir).unwrap();

        let config_path = moss_dir.join("config.toml");
        let mut file = std::fs::File::create(&config_path).unwrap();
        writeln!(
            file,
            r#"
[pretty]
enabled = true
colors = "always"
highlight = false
"#
        )
        .unwrap();

        let config = NormalizeConfig::load(dir.path());
        assert_eq!(config.pretty.enabled, Some(true));
        assert_eq!(config.pretty.colors, Some(ColorMode::Always));
        assert_eq!(config.pretty.highlight, Some(false));
        assert!(!config.pretty.highlight());
    }

    #[test]
    fn test_view_context_files_default() {
        let dir = TempDir::new().unwrap();
        let config = NormalizeConfig::load(dir.path());
        // Default: ["SUMMARY.md", ".context.md"]
        assert_eq!(
            config.view.context_files(),
            vec!["SUMMARY.md", ".context.md"]
        );
    }

    #[test]
    fn test_view_context_files_custom() {
        let dir = TempDir::new().unwrap();
        let moss_dir = dir.path().join(".normalize");
        std::fs::create_dir_all(&moss_dir).unwrap();

        let config_path = moss_dir.join("config.toml");
        std::fs::write(
            &config_path,
            "[view]\ncontext_files = [\"README.md\", \"SUMMARY.md\"]\n",
        )
        .unwrap();

        let config = NormalizeConfig::load(dir.path());
        assert_eq!(config.view.context_files(), vec!["README.md", "SUMMARY.md"]);
    }
}
