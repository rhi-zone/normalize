//! Rules management service for server-less CLI.

use normalize_output::OutputFormatter;
use normalize_output::diagnostics::DiagnosticsReport;
use server_less::cli;
use std::cell::Cell;
use std::path::Path;

use normalize_syntax_rules::apply_fixes;

use crate::cmd_rules::run_syntax_rules;
use crate::runner::{
    ListFilters, RuleType, RulesListReport, RulesRunConfig, add_rule, apply_native_rules_config,
    build_list_report, enable_disable, list_tags, remove_rule, run_rules_report, show_rule,
    update_rules,
};

/// Resolve pretty mode: enabled on TTY (or forced via --pretty), disabled by --compact.
fn resolve_pretty(pretty: bool, compact: bool) -> bool {
    use std::io::IsTerminal;
    !compact && (pretty || std::io::stdout().is_terminal())
}

/// Report returned by `normalize rules validate`.
#[derive(serde::Serialize, schemars::JsonSchema)]
pub struct RulesValidateReport {
    /// Path to the config file that was checked.
    pub config_path: String,
    /// Overall validity — false if any errors were found.
    pub valid: bool,
    /// TOML parse errors or unknown rule IDs.
    pub errors: Vec<String>,
    /// Non-fatal warnings (e.g. disabled rules that have no effect).
    pub warnings: Vec<String>,
    /// Number of per-rule overrides found in the config.
    pub rule_count: usize,
    /// Number of global-allow patterns.
    pub global_allow_count: usize,
}

impl OutputFormatter for RulesValidateReport {
    fn format_text(&self) -> String {
        let mut out = String::new();
        if self.valid {
            out.push_str("Rules configuration is valid\n");
        } else {
            out.push_str("Rules configuration has errors\n");
        }
        out.push('\n');
        out.push_str(&format!("Config file: {}\n", self.config_path));

        if self.valid {
            out.push_str(&format!(
                "  {} rule override{}, {} global-allow pattern{}\n",
                self.rule_count,
                if self.rule_count == 1 { "" } else { "s" },
                self.global_allow_count,
                if self.global_allow_count == 1 {
                    ""
                } else {
                    "s"
                },
            ));
        } else {
            out.push_str(&format!(
                "  {} error{}:\n\n",
                self.errors.len(),
                if self.errors.len() == 1 { "" } else { "s" }
            ));
            for e in &self.errors {
                out.push_str(&format!("  error: {e}\n"));
            }
        }

        if !self.warnings.is_empty() {
            out.push('\n');
            for w in &self.warnings {
                out.push_str(&format!("  warning: {w}\n"));
            }
        }

        out
    }
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
pub struct RuleShowReport {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

impl OutputFormatter for RuleShowReport {
    fn format_text(&self) -> String {
        if let Some(ref msg) = self.message {
            msg.clone()
        } else if self.success {
            "Done".to_string()
        } else {
            "Failed".to_string()
        }
    }
}

impl std::fmt::Display for RuleShowReport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.format_text())
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

    fn display_validate(&self, r: &RulesValidateReport) -> String {
        if self.pretty.get() {
            r.format_pretty()
        } else {
            r.format_text()
        }
    }
}

#[cli(
    name = "rules",
    description = "Manage and run analysis rules (syntax + fact + native)",
    global = [
        pretty = "Human-friendly output with colors and formatting",
        compact = "Compact output without colors (overrides TTY detection)",
    ]
)]
impl RulesService {
    /// List all rules (syntax + fact, builtin + user)
    ///
    /// Examples:
    ///   normalize rules list                   # all rules with status
    ///   normalize rules list --pretty          # with full descriptions
    ///   normalize rules list --json            # machine-readable output
    #[cli(display_with = "display_list")]
    #[allow(clippy::too_many_arguments)]
    pub fn list(
        &self,
        #[param(short = 't', help = "Filter by rule type (all, syntax, fact)")] r#type: Option<
            String,
        >,
        #[param(help = "Filter by tag")] tag: Option<String>,
        #[param(help = "Filter to enabled rules only")] enabled: bool,
        #[param(help = "Filter to disabled rules only")] disabled: bool,
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
                type_filter: &rule_type,
                tag: tag.as_deref(),
                enabled,
                disabled,
            },
            &config,
        );
        Ok(report)
    }

    /// Run rules against the codebase
    ///
    /// Examples:
    ///   normalize rules run                    # run all enabled rules
    ///   normalize rules run src/               # run on specific directory
    ///   normalize rules run --rule rust/unwrap-in-impl   # single rule
    ///   normalize rules run --pretty           # colored output with details
    ///   normalize rules run --type syntax      # only syntax rules
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

        // --fix path: syntax-only, loop until no fixable issues remain
        if fix {
            let debug_flags = normalize_syntax_rules::DebugFlags::from_args(&debug);
            tokio::task::spawn_blocking(move || {
                let mut total_fixed = 0usize;
                let mut total_files = 0usize;
                loop {
                    let findings = run_syntax_rules(
                        &target_root,
                        &project_root,
                        rule.as_deref(),
                        tag.as_deref(),
                        None,
                        &config.rules,
                        &debug_flags,
                    );
                    let fixable_count = findings.iter().filter(|f| f.fix.is_some()).count();
                    if fixable_count == 0 {
                        break;
                    }
                    match apply_fixes(&findings) {
                        Ok(0) => break,
                        Ok(files_modified) => {
                            total_fixed += fixable_count;
                            total_files = files_modified;
                        }
                        Err(e) => return Err(format!("Error applying fixes: {e}")),
                    }
                }
                if total_fixed == 0 {
                    eprintln!("No auto-fixable issues found.");
                } else {
                    println!("Fixed {} issue(s) in {} file(s).", total_fixed, total_files);
                }
                Ok(())
            })
            .await
            .map_err(|e| format!("Task error: {e}"))??;
            return Ok(DiagnosticsReport::new());
        }

        let run_native = matches!(rule_type, RuleType::All | RuleType::Native);
        let rule_filter = rule.clone();

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

        // Native engine (stale-summary, check-refs, stale-docs, check-examples, ratchet, budget)
        // runs in async context; included in All and Native engine types.
        // All checks are independent — run them in parallel.
        if run_native {
            let native_root = effective_root.clone();
            let native_config = load_rules_config(&native_root);
            let threshold = 10;

            let (summary_res, stale_res, examples_res, refs_res, ratchet_res, budget_res) = tokio::join!(
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
                tokio::task::spawn_blocking({
                    let root = native_root.clone();
                    move || normalize_native_rules::build_ratchet_report(&root)
                }),
                tokio::task::spawn_blocking({
                    let root = native_root.clone();
                    move || normalize_native_rules::build_budget_report(&root)
                }),
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
            if let Ok(r) = ratchet_res {
                report.merge(r.into());
            }
            if let Ok(r) = budget_res {
                report.merge(r.into());
            }

            apply_native_rules_config(&mut report, &native_config.rules);
            report.sources_run.push("native".into());
        }

        // Apply --rule filter across all engines (syntax/fact already filter internally,
        // but native engine produces all its issues unconditionally).
        if let Some(ref filter) = rule_filter {
            report
                .issues
                .retain(|issue| issue.rule_id == filter.as_str());
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
    ///
    /// Examples:
    ///   normalize rules enable python/bare-except   # enable a specific rule
    ///   normalize rules enable --tag correctness    # enable all correctness rules
    pub fn enable(
        &self,
        #[param(positional, help = "Rule ID or tag name")] id_or_tag: String,
        #[param(help = "Preview changes without writing")] dry_run: bool,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
    ) -> Result<RuleShowReport, String> {
        let effective_root = root
            .as_deref()
            .map(std::path::PathBuf::from)
            .map(Ok)
            .unwrap_or_else(std::env::current_dir)
            .map_err(|e| format!("Failed to get current directory: {e}"))?;
        let config = load_rules_config(&effective_root);
        enable_disable(&effective_root, &id_or_tag, true, dry_run, &config).map(|msg| {
            RuleShowReport {
                success: true,
                message: Some(msg),
            }
        })
    }

    /// Disable a rule or all rules matching a tag
    ///
    /// Examples:
    ///   normalize rules disable no-todo-comment     # disable a specific rule
    pub fn disable(
        &self,
        #[param(positional, help = "Rule ID or tag name")] id_or_tag: String,
        #[param(help = "Preview changes without writing")] dry_run: bool,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
    ) -> Result<RuleShowReport, String> {
        let effective_root = root
            .as_deref()
            .map(std::path::PathBuf::from)
            .map(Ok)
            .unwrap_or_else(std::env::current_dir)
            .map_err(|e| format!("Failed to get current directory: {e}"))?;
        let config = load_rules_config(&effective_root);
        enable_disable(&effective_root, &id_or_tag, false, dry_run, &config).map(|msg| {
            RuleShowReport {
                success: true,
                message: Some(msg),
            }
        })
    }

    /// Show full documentation for a rule
    ///
    /// Examples:
    ///   normalize rules show rust/unwrap-in-impl    # full docs for a rule
    pub fn show(
        &self,
        #[param(positional, help = "Rule ID to show")] id: String,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
        pretty: bool,
        compact: bool,
    ) -> Result<RuleShowReport, String> {
        let effective_root = root
            .as_deref()
            .map(std::path::PathBuf::from)
            .map(Ok)
            .unwrap_or_else(std::env::current_dir)
            .map_err(|e| format!("Failed to get current directory: {e}"))?;
        let use_colors = resolve_pretty(pretty, compact);
        let config = load_rules_config(&effective_root);
        show_rule(&effective_root, &id, use_colors, &config).map(|msg| RuleShowReport {
            success: true,
            message: Some(msg),
        })
    }

    /// List all tags and the rules they group
    ///
    /// Examples:
    ///   normalize rules tags                   # list all tags with rule counts
    pub fn tags(
        &self,
        #[param(help = "Expand each tag to show its member rules")] show_rules: bool,
        #[param(help = "Show only this specific tag")] tag: Option<String>,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
        pretty: bool,
        compact: bool,
    ) -> Result<RuleShowReport, String> {
        let effective_root = root
            .as_deref()
            .map(std::path::PathBuf::from)
            .map(Ok)
            .unwrap_or_else(std::env::current_dir)
            .map_err(|e| format!("Failed to get current directory: {e}"))?;
        let use_colors = resolve_pretty(pretty, compact);
        let config = load_rules_config(&effective_root);
        list_tags(
            &effective_root,
            show_rules,
            tag.as_deref(),
            use_colors,
            &config,
        )
        .map(|msg| RuleShowReport {
            success: true,
            message: Some(msg),
        })
    }

    /// Add a rule from a URL
    ///
    /// Examples:
    ///   normalize rules add https://example.com/rule.scm   # import a rule from URL
    pub fn add(
        &self,
        #[param(positional, help = "URL to download the rule from")] url: String,
        #[param(help = "Install to global rules instead of project")] global: bool,
    ) -> Result<RuleShowReport, String> {
        add_rule(&url, global).map(|_| RuleShowReport {
            success: true,
            message: None,
        })
    }

    /// Update imported rules from their sources
    pub fn update(
        &self,
        #[param(
            positional,
            help = "Specific rule ID to update (updates all if omitted)"
        )]
        rule_id: Option<String>,
    ) -> Result<RuleShowReport, String> {
        update_rules(rule_id.as_deref()).map(|_| RuleShowReport {
            success: true,
            message: None,
        })
    }

    /// Remove an imported rule
    pub fn remove(
        &self,
        #[param(positional, help = "Rule ID to remove")] rule_id: String,
    ) -> Result<RuleShowReport, String> {
        remove_rule(&rule_id).map(|_| RuleShowReport {
            success: true,
            message: None,
        })
    }

    /// Interactive setup wizard — run all rules and walk through enable/disable decisions
    ///
    /// Examples:
    ///   normalize rules setup                    # interactive rule configuration
    pub fn setup(
        &self,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
    ) -> Result<RuleShowReport, String> {
        let effective_root = root
            .as_deref()
            .map(std::path::PathBuf::from)
            .map(Ok)
            .unwrap_or_else(std::env::current_dir)
            .map_err(|e| format!("Failed to get current directory: {e}"))?;
        let exit_code = crate::setup::run_setup_wizard(&effective_root);
        if exit_code == 0 {
            Ok(RuleShowReport {
                success: true,
                message: None,
            })
        } else {
            Err("Setup wizard failed".to_string())
        }
    }

    /// Validate the rules configuration — check rule IDs, TOML syntax, and report issues
    ///
    /// Examples:
    ///   normalize rules validate               # check rule config for errors
    #[cli(display_with = "display_validate")]
    pub fn validate(
        &self,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
        pretty: bool,
        compact: bool,
    ) -> Result<RulesValidateReport, String> {
        let effective_root = root
            .as_deref()
            .map(std::path::PathBuf::from)
            .map(Ok)
            .unwrap_or_else(std::env::current_dir)
            .map_err(|e| format!("Failed to get current directory: {e}"))?;
        self.pretty.set(resolve_pretty(pretty, compact));

        let config_file = effective_root.join(".normalize").join("config.toml");
        let config_path = if config_file.exists() {
            ".normalize/config.toml".to_string()
        } else {
            "(not found — using defaults)".to_string()
        };

        let mut errors: Vec<String> = Vec::new();

        // Check for TOML parse errors directly (load_rules_config silently swallows them)
        if config_file.exists() {
            let raw = std::fs::read_to_string(&config_file)
                .map_err(|e| format!("Failed to read config: {e}"))?;
            if let Err(e) = toml::from_str::<toml::Value>(&raw) {
                errors.push(format!("TOML parse error: {e}"));
                return Ok(RulesValidateReport {
                    config_path,
                    valid: false,
                    errors,
                    warnings: Vec::new(),
                    rule_count: 0,
                    global_allow_count: 0,
                });
            }
        }

        // Load effective config
        let config = load_rules_config(&effective_root);
        let rules_cfg = &config.rules;

        let rule_count = rules_cfg.rules.len();
        let global_allow_count = rules_cfg.global_allow.len();

        // Build list of known rule IDs by querying all rule engines
        let list_report = build_list_report(
            &effective_root,
            &ListFilters {
                type_filter: &RuleType::All,
                tag: None,
                enabled: false,
                disabled: false,
            },
            &config,
        );
        let known_ids: std::collections::HashSet<String> =
            list_report.rules.iter().map(|r| r.id.clone()).collect();

        // Check each configured rule ID against known rules
        for rule_id in rules_cfg.rules.keys() {
            if !known_ids.contains(rule_id) {
                errors.push(format!(
                    "unknown rule ID \"{rule_id}\" — run 'normalize rules list' to see available rules"
                ));
            }
        }

        let valid = errors.is_empty();

        let report = RulesValidateReport {
            config_path,
            valid,
            errors,
            warnings: Vec::new(),
            rule_count,
            global_allow_count,
        };

        if !report.valid {
            return Err(report.format_text());
        }

        Ok(report)
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
