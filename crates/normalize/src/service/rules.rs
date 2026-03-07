//! Rules management service for server-less CLI.

use super::resolve_pretty;
use server_less::cli;
use std::cell::Cell;
use std::path::Path;

/// Rules management sub-service.
pub struct RulesService {
    _pretty: Cell<bool>,
}

impl RulesService {
    pub fn new(pretty: &Cell<bool>) -> Self {
        Self {
            _pretty: Cell::new(pretty.get()),
        }
    }
}

/// Generic result type for rule operations.
#[derive(serde::Serialize, schemars::JsonSchema)]
pub struct RuleResult {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

impl std::fmt::Display for RuleResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(ref msg) = self.message {
            write!(f, "{}", msg)
        } else if self.success {
            write!(f, "Done")
        } else {
            write!(f, "Failed")
        }
    }
}

#[cli(
    name = "rules",
    about = "Manage and run analysis rules (syntax + fact)"
)]
impl RulesService {
    /// List all rules (syntax + fact, builtin + user)
    #[allow(clippy::too_many_arguments)]
    pub fn list(
        &self,
        #[param(help = "Show source URLs for imported rules")] sources: bool,
        #[param(short = 't', help = "Filter by rule type (all, syntax, fact)")] r#type: Option<
            String,
        >,
        #[param(help = "Filter by tag")] tag: Option<String>,
        #[param(help = "Filter to enabled rules only")] enabled: bool,
        #[param(help = "Filter to disabled rules only")] disabled: bool,
        #[param(help = "Hide the description line")] no_desc: bool,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
        pretty: bool,
        compact: bool,
    ) -> Result<RuleResult, String> {
        let effective_root = root
            .as_deref()
            .map(std::path::PathBuf::from)
            .map(Ok)
            .unwrap_or_else(std::env::current_dir)
            .map_err(|e| format!("Failed to get current directory: {e}"))?;
        let use_colors = resolve_pretty(
            root.as_deref().map(Path::new).unwrap_or(Path::new(".")),
            pretty,
            compact,
        );
        let config = crate::config::NormalizeConfig::load(&effective_root);
        let rule_type: crate::commands::rules::RuleType = r#type
            .as_deref()
            .unwrap_or("all")
            .parse()
            .unwrap_or_default();
        let exit_code = crate::commands::rules::cmd_list(
            &effective_root,
            crate::commands::rules::ListFilters {
                sources,
                type_filter: &rule_type,
                tag: tag.as_deref(),
                enabled,
                disabled,
                no_desc,
                json: false,
                use_colors,
            },
            &config,
        );
        crate::commands::rules::exit_to_result(exit_code)
    }

    /// Run rules against the codebase
    #[allow(clippy::too_many_arguments)]
    pub async fn run(
        &self,
        #[param(help = "Specific rule ID to run")] rule: Option<String>,
        #[param(help = "Filter by tag")] tag: Option<String>,
        #[param(help = "Apply auto-fixes (syntax rules only)")] fix: bool,
        #[param(help = "Output in SARIF format")] sarif: bool,
        #[param(positional, help = "Target directory or file")] target: Option<String>,
        #[param(short = 't', help = "Filter by rule type (all, syntax, fact)")] r#type: Option<
            String,
        >,
        #[param(help = "Debug flags (comma-separated)")] debug: Vec<String>,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
    ) -> Result<RuleResult, String> {
        let effective_root = root
            .as_deref()
            .map(std::path::PathBuf::from)
            .map(Ok)
            .unwrap_or_else(std::env::current_dir)
            .map_err(|e| format!("Failed to get current directory: {e}"))?;
        let config = crate::config::NormalizeConfig::load(&effective_root);
        let rule_type: crate::commands::rules::RuleType = r#type
            .as_deref()
            .unwrap_or("all")
            .parse()
            .unwrap_or_default();
        let target_root = target
            .as_deref()
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| effective_root.clone());
        let project_root = effective_root.clone();
        // cmd_run internally creates a tokio Runtime for fact rules; spawn_blocking
        // gives it a thread without an active runtime so block_on() doesn't panic.
        let exit_code = tokio::task::spawn_blocking(move || {
            crate::commands::rules::cmd_run(
                &target_root,
                &project_root,
                rule.as_deref(),
                tag.as_deref(),
                fix,
                sarif,
                &rule_type,
                &debug,
                false,
                &config,
            )
        })
        .await
        .map_err(|e| format!("Task error: {e}"))?;
        crate::commands::rules::exit_to_result(exit_code)
    }

    /// Enable a rule or all rules matching a tag
    pub fn enable(
        &self,
        #[param(positional, help = "Rule ID or tag name")] id_or_tag: String,
        #[param(help = "Preview changes without writing")] dry_run: bool,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
    ) -> Result<RuleResult, String> {
        let effective_root = root
            .as_deref()
            .map(std::path::PathBuf::from)
            .map(Ok)
            .unwrap_or_else(std::env::current_dir)
            .map_err(|e| format!("Failed to get current directory: {e}"))?;
        let config = crate::config::NormalizeConfig::load(&effective_root);
        let exit_code = crate::commands::rules::cmd_enable_disable(
            &effective_root,
            &id_or_tag,
            true,
            dry_run,
            &config,
        );
        crate::commands::rules::exit_to_result(exit_code)
    }

    /// Disable a rule or all rules matching a tag
    pub fn disable(
        &self,
        #[param(positional, help = "Rule ID or tag name")] id_or_tag: String,
        #[param(help = "Preview changes without writing")] dry_run: bool,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
    ) -> Result<RuleResult, String> {
        let effective_root = root
            .as_deref()
            .map(std::path::PathBuf::from)
            .map(Ok)
            .unwrap_or_else(std::env::current_dir)
            .map_err(|e| format!("Failed to get current directory: {e}"))?;
        let config = crate::config::NormalizeConfig::load(&effective_root);
        let exit_code = crate::commands::rules::cmd_enable_disable(
            &effective_root,
            &id_or_tag,
            false,
            dry_run,
            &config,
        );
        crate::commands::rules::exit_to_result(exit_code)
    }

    /// Show full documentation for a rule
    pub fn show(
        &self,
        #[param(positional, help = "Rule ID to show")] id: String,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
        pretty: bool,
        compact: bool,
    ) -> Result<RuleResult, String> {
        let effective_root = root
            .as_deref()
            .map(std::path::PathBuf::from)
            .map(Ok)
            .unwrap_or_else(std::env::current_dir)
            .map_err(|e| format!("Failed to get current directory: {e}"))?;
        let use_colors = resolve_pretty(
            root.as_deref().map(Path::new).unwrap_or(Path::new(".")),
            pretty,
            compact,
        );
        let config = crate::config::NormalizeConfig::load(&effective_root);
        let exit_code =
            crate::commands::rules::cmd_show(&effective_root, &id, false, use_colors, &config);
        crate::commands::rules::exit_to_result(exit_code)
    }

    /// List all tags and the rules they group
    pub fn tags(
        &self,
        #[param(help = "Expand each tag to show its member rules")] show_rules: bool,
        #[param(help = "Show only this specific tag")] tag: Option<String>,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
        pretty: bool,
        compact: bool,
    ) -> Result<RuleResult, String> {
        let effective_root = root
            .as_deref()
            .map(std::path::PathBuf::from)
            .map(Ok)
            .unwrap_or_else(std::env::current_dir)
            .map_err(|e| format!("Failed to get current directory: {e}"))?;
        let use_colors = resolve_pretty(
            root.as_deref().map(Path::new).unwrap_or(Path::new(".")),
            pretty,
            compact,
        );
        let config = crate::config::NormalizeConfig::load(&effective_root);
        let exit_code = crate::commands::rules::cmd_tags(
            &effective_root,
            show_rules,
            tag.as_deref(),
            false,
            use_colors,
            &config,
        );
        crate::commands::rules::exit_to_result(exit_code)
    }

    /// Add a rule from a URL
    pub fn add(
        &self,
        #[param(positional, help = "URL to download the rule from")] url: String,
        #[param(help = "Install to global rules instead of project")] global: bool,
    ) -> Result<RuleResult, String> {
        let exit_code = crate::commands::rules::cmd_add(&url, global, false);
        crate::commands::rules::exit_to_result(exit_code)
    }

    /// Update imported rules from their sources
    pub fn update(
        &self,
        #[param(
            positional,
            help = "Specific rule ID to update (updates all if omitted)"
        )]
        rule_id: Option<String>,
    ) -> Result<RuleResult, String> {
        let exit_code = crate::commands::rules::cmd_update(rule_id.as_deref(), false);
        crate::commands::rules::exit_to_result(exit_code)
    }

    /// Remove an imported rule
    pub fn remove(
        &self,
        #[param(positional, help = "Rule ID to remove")] rule_id: String,
    ) -> Result<RuleResult, String> {
        let exit_code = crate::commands::rules::cmd_remove(&rule_id, false);
        crate::commands::rules::exit_to_result(exit_code)
    }
}
