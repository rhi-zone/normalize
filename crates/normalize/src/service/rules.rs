//! Rules management service for server-less CLI.

use super::resolve_pretty;
use normalize_output::OutputFormatter;
use normalize_output::diagnostics::DiagnosticsReport;
use server_less::cli;
use std::cell::Cell;
use std::path::Path;

/// Rules management sub-service.
pub struct RulesService {
    pretty: Cell<bool>,
    /// Set to true when --sarif is active; display_run emits SARIF instead of text.
    sarif: Cell<bool>,
    /// Issue display limit for text output (default 50).
    limit: Cell<usize>,
}

impl RulesService {
    pub fn new(pretty: &Cell<bool>) -> Self {
        Self {
            pretty: Cell::new(pretty.get()),
            sarif: Cell::new(false),
            limit: Cell::new(50),
        }
    }

    fn resolve_format(&self, pretty: bool, compact: bool, root: &Path) {
        self.pretty.set(resolve_pretty(root, pretty, compact));
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

impl RulesService {
    fn display_run(&self, r: &DiagnosticsReport) -> String {
        if self.sarif.get() {
            return r.format_sarif();
        }
        if self.pretty.get() {
            r.format_pretty()
        } else {
            r.format_text_limited(Some(self.limit.get()))
        }
    }
}

#[cli(
    name = "rules",
    about = "Manage and run analysis rules (syntax + fact + native)"
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
    #[cli(display_with = "display_run")]
    #[allow(clippy::too_many_arguments)]
    pub async fn run(
        &self,
        #[param(help = "Specific rule ID to run")] rule: Option<String>,
        #[param(help = "Filter by tag")] tag: Option<String>,
        #[param(help = "Apply auto-fixes (syntax rules only)")] fix: bool,
        #[param(help = "Output in SARIF format")] sarif: bool,
        #[param(positional, help = "Target directory or file")] target: Option<String>,
        #[param(
            short = 't',
            help = "Filter by rule type (all, syntax, fact, native, sarif)"
        )]
        r#type: Option<String>,
        #[param(help = "Debug flags (comma-separated)")] debug: Vec<String>,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
        #[param(help = "Exit 0 even when error-severity issues are found")] no_fail: bool,
        #[param(help = "Maximum number of issues to show in detail (default: 50)")] limit: Option<
            usize,
        >,
        pretty: bool,
        compact: bool,
    ) -> Result<DiagnosticsReport, String> {
        let effective_root = root
            .as_deref()
            .map(std::path::PathBuf::from)
            .map(Ok)
            .unwrap_or_else(std::env::current_dir)
            .map_err(|e| format!("Failed to get current directory: {e}"))?;
        self.resolve_format(
            pretty,
            compact,
            root.as_deref().map(Path::new).unwrap_or(Path::new(".")),
        );
        self.sarif.set(sarif);
        self.limit.set(limit.unwrap_or(50));
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

        // --fix path: syntax-only, keep existing fix loop behaviour
        if fix {
            let debug_flags = normalize_syntax_rules::DebugFlags::from_args(&debug);
            let exit_code = tokio::task::spawn_blocking(move || {
                crate::commands::analyze::rules_cmd::cmd_rules(
                    &target_root,
                    &project_root,
                    rule.as_deref(),
                    tag.as_deref(),
                    None,
                    false,
                    true,
                    false,
                    &config.analyze.rules,
                    &debug_flags,
                )
            })
            .await
            .map_err(|e| format!("Task error: {e}"))?;
            return if exit_code == 0 {
                Ok(DiagnosticsReport::new())
            } else {
                Err("Fix failed".to_string())
            };
        }

        let run_native = matches!(
            rule_type,
            crate::commands::rules::RuleType::All | crate::commands::rules::RuleType::Native
        );

        // Syntax + fact + SARIF engines via run_rules_report() (blocking)
        let mut report = tokio::task::spawn_blocking(move || {
            crate::commands::rules::run_rules_report(
                &target_root,
                &project_root,
                rule.as_deref(),
                tag.as_deref(),
                &rule_type,
                &debug,
                &config,
            )
        })
        .await
        .map_err(|e| format!("Task error: {e}"))?;

        // Native engine (stale-summary, check-refs, stale-docs, check-examples)
        // runs in async context; included in All and Native engine types.
        if run_native {
            let native_root = effective_root.clone();
            let native_config = crate::config::NormalizeConfig::load(&native_root);
            let threshold = 10;

            let summary_report =
                crate::commands::analyze::stale_summary::build_stale_summary_report(
                    &native_root,
                    threshold,
                );
            report.merge(summary_report.into());

            let stale_report =
                crate::commands::analyze::stale_docs::build_stale_docs_report(&native_root);
            report.merge(stale_report.into());

            let examples_report =
                crate::commands::analyze::check_examples::build_check_examples_report(&native_root);
            report.merge(examples_report.into());

            if let Ok(refs_report) =
                crate::commands::analyze::check_refs::build_check_refs_report(&native_root).await
            {
                report.merge(refs_report.into());
            }

            crate::commands::rules::apply_native_rules_config(
                &mut report,
                &native_config.analyze.rules,
            );
            report.sources_run.push("native".into());
        }

        report.sort();

        let error_count = report.count_by_severity(normalize_output::diagnostics::Severity::Error);
        if !no_fail && error_count > 0 {
            let detail = self.display_run(&report);
            return Err(format!("{detail}\n{error_count} error(s) found"));
        }

        Ok(report)
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
