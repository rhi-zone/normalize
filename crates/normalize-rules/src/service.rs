//! Rules management service for server-less CLI.

use normalize_output::OutputFormatter;
use normalize_output::diagnostics::DiagnosticsReport;
use server_less::cli;
use std::cell::Cell;
use std::path::Path;

use crate::runner::{
    ListFilters, RuleType, RulesListReport, RulesRunConfig, apply_native_rules_config,
    build_list_report, cmd_add, cmd_enable_disable, cmd_remove, cmd_show, cmd_tags, cmd_update,
    exit_to_result, run_rules_report,
};

/// Resolve pretty mode: enabled on TTY (or forced via --pretty), disabled by --compact.
fn resolve_pretty(pretty: bool, compact: bool) -> bool {
    use std::io::IsTerminal;
    !compact && (pretty || std::io::stdout().is_terminal())
}

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

    fn resolve_format(&self, pretty: bool, compact: bool) {
        self.pretty.set(resolve_pretty(pretty, compact));
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

    fn display_list(&self, r: &RulesListReport) -> String {
        if self.pretty.get() {
            r.format_pretty()
        } else {
            r.format_text()
        }
    }
}

#[cli(
    name = "rules",
    about = "Manage and run analysis rules (syntax + fact + native)"
)]
impl RulesService {
    /// List all rules (syntax + fact, builtin + user)
    #[cli(display_with = "display_list")]
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
    ) -> Result<RulesListReport, String> {
        let effective_root = root
            .as_deref()
            .map(std::path::PathBuf::from)
            .map(Ok)
            .unwrap_or_else(std::env::current_dir)
            .map_err(|e| format!("Failed to get current directory: {e}"))?;
        self.resolve_format(pretty, compact);
        let config = load_rules_config(&effective_root);
        let rule_type: RuleType = r#type
            .as_deref()
            .unwrap_or("all")
            .parse()
            .unwrap_or_default();
        let report = build_list_report(
            &effective_root,
            &ListFilters {
                sources,
                type_filter: &rule_type,
                tag: tag.as_deref(),
                enabled,
                disabled,
                no_desc,
                json: false,
                use_colors: false, // not used by build_list_report
            },
            &config,
        );
        Ok(report)
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
        self.resolve_format(pretty, compact);
        self.sarif.set(sarif);
        self.limit.set(limit.unwrap_or(50));
        let config = load_rules_config(&effective_root);
        let rule_type: RuleType = r#type
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
                crate::cmd_rules::cmd_rules(
                    &target_root,
                    &project_root,
                    rule.as_deref(),
                    tag.as_deref(),
                    None,
                    false,
                    true,
                    false,
                    &config.rules,
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

        let run_native = matches!(rule_type, RuleType::All | RuleType::Native);

        // Syntax + fact + SARIF engines via run_rules_report() (blocking)
        let mut report = tokio::task::spawn_blocking(move || {
            run_rules_report(
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
        // All four checks are independent — run them in parallel.
        if run_native {
            let native_root = effective_root.clone();
            let native_config = load_rules_config(&native_root);
            let threshold = 10;

            let (summary_res, stale_res, examples_res, refs_res) = tokio::join!(
                tokio::task::spawn_blocking({
                    let root = native_root.clone();
                    move || normalize_native_rules::build_stale_summary_report(&root, threshold)
                }),
                tokio::task::spawn_blocking({
                    let root = native_root.clone();
                    move || normalize_native_rules::build_stale_docs_report(&root)
                }),
                tokio::task::spawn_blocking({
                    let root = native_root.clone();
                    move || normalize_native_rules::build_check_examples_report(&root)
                }),
                normalize_native_rules::build_check_refs_report(&native_root),
            );
            if let Ok(r) = summary_res {
                report.merge(r.into());
            }
            if let Ok(r) = stale_res {
                report.merge(r.into());
            }
            if let Ok(r) = examples_res {
                report.merge(r.into());
            }
            if let Ok(r) = refs_res {
                report.merge(r.into());
            }

            apply_native_rules_config(&mut report, &native_config.rules);
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
        let config = load_rules_config(&effective_root);
        let exit_code = cmd_enable_disable(&effective_root, &id_or_tag, true, dry_run, &config);
        exit_to_result(exit_code)
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
        let config = load_rules_config(&effective_root);
        let exit_code = cmd_enable_disable(&effective_root, &id_or_tag, false, dry_run, &config);
        exit_to_result(exit_code)
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
        let use_colors = resolve_pretty(pretty, compact);
        let config = load_rules_config(&effective_root);
        let exit_code = cmd_show(&effective_root, &id, false, use_colors, &config);
        exit_to_result(exit_code)
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
        let use_colors = resolve_pretty(pretty, compact);
        let config = load_rules_config(&effective_root);
        let exit_code = cmd_tags(
            &effective_root,
            show_rules,
            tag.as_deref(),
            false,
            use_colors,
            &config,
        );
        exit_to_result(exit_code)
    }

    /// Add a rule from a URL
    pub fn add(
        &self,
        #[param(positional, help = "URL to download the rule from")] url: String,
        #[param(help = "Install to global rules instead of project")] global: bool,
    ) -> Result<RuleResult, String> {
        let exit_code = cmd_add(&url, global, false);
        exit_to_result(exit_code)
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
        let exit_code = cmd_update(rule_id.as_deref(), false);
        exit_to_result(exit_code)
    }

    /// Remove an imported rule
    pub fn remove(
        &self,
        #[param(positional, help = "Rule ID to remove")] rule_id: String,
    ) -> Result<RuleResult, String> {
        let exit_code = cmd_remove(&rule_id, false);
        exit_to_result(exit_code)
    }
}

/// Load `RulesRunConfig` from the project config files.
///
/// This mirrors the structure of `NormalizeConfig` but only pulls out the
/// fields needed by the rules machinery, avoiding a circular dependency on the
/// main `normalize` crate.
pub fn load_rules_config(root: &Path) -> RulesRunConfig {
    // We parse a minimal subset of normalize.toml — just the rules-related sections.
    let project_path = root.join(".normalize").join("config.toml");
    let content = std::fs::read_to_string(&project_path).unwrap_or_default();

    #[derive(serde::Deserialize, Default)]
    #[serde(default)]
    struct RulesOnlyConfig {
        rules: crate::runner::RulesConfig,
        #[serde(rename = "rule-tags")]
        rule_tags: std::collections::HashMap<String, Vec<String>>,
    }

    // Load global config first
    let global_content = dirs::config_dir()
        .map(|d| d.join("normalize").join("config.toml"))
        .and_then(|p| std::fs::read_to_string(p).ok())
        .unwrap_or_default();

    let global: RulesOnlyConfig = toml::from_str(&global_content).unwrap_or_default();
    let project: RulesOnlyConfig = toml::from_str(&content).unwrap_or_default();

    let rule_tags = {
        let mut merged = global.rule_tags;
        merged.extend(project.rule_tags);
        merged
    };

    // Project config wins for rules (same semantics as normalize's config merge)
    let rules = if content.is_empty() {
        global.rules
    } else {
        project.rules
    };
    RulesRunConfig { rule_tags, rules }
}
