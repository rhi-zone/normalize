//! Rules management service for server-less CLI.

use server_less::cli;
use std::cell::Cell;

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
        crate::commands::rules::cmd_list_service(
            root.as_deref(),
            sources,
            r#type.as_deref().unwrap_or("all"),
            tag.as_deref(),
            enabled,
            disabled,
            no_desc,
            !compact && pretty,
        )
    }

    /// Run rules against the codebase
    #[allow(clippy::too_many_arguments)]
    pub fn run(
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
        crate::commands::rules::cmd_run_service(
            root.as_deref(),
            rule.as_deref(),
            tag.as_deref(),
            fix,
            sarif,
            target.as_deref(),
            r#type.as_deref().unwrap_or("all"),
            &debug,
        )
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
        crate::commands::rules::cmd_enable_disable_service(
            root.as_deref(),
            &id_or_tag,
            true,
            dry_run,
        )
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
        crate::commands::rules::cmd_enable_disable_service(
            root.as_deref(),
            &id_or_tag,
            false,
            dry_run,
        )
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
        crate::commands::rules::cmd_show_service(root.as_deref(), &id, !compact && pretty)
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
        crate::commands::rules::cmd_tags_service(
            root.as_deref(),
            show_rules,
            tag.as_deref(),
            !compact && pretty,
        )
    }

    /// Add a rule from a URL
    pub fn add(
        &self,
        #[param(positional, help = "URL to download the rule from")] url: String,
        #[param(help = "Install to global rules instead of project")] global: bool,
    ) -> Result<RuleResult, String> {
        crate::commands::rules::cmd_add_service(&url, global)
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
        crate::commands::rules::cmd_update_service(rule_id.as_deref())
    }

    /// Remove an imported rule
    pub fn remove(
        &self,
        #[param(positional, help = "Rule ID to remove")] rule_id: String,
    ) -> Result<RuleResult, String> {
        crate::commands::rules::cmd_remove_service(&rule_id)
    }
}
