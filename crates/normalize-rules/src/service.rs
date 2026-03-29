//! Rules management service for server-less CLI.

use normalize_output::OutputFormatter;
use normalize_output::diagnostics::DiagnosticsReport;
use server_less::cli;
use std::cell::Cell;
use std::path::Path;

use normalize_syntax_rules::apply_fixes;

use crate::cmd_rules::run_syntax_rules;
use crate::runner::{
    ListFilters, RuleInfoReport, RuleKind, RulesListReport, RulesRunConfig, RulesTagsReport,
    add_rule, apply_native_rules_config, build_list_report, enable_disable, list_tags_structured,
    remove_rule, run_rules_report, show_rule_structured, update_rules,
};

/// Typed config for summary rules (stale-summary, missing-summary).
/// Deserialized from `extra` fields on the `RuleOverride` via `rule_config()`.
#[derive(serde::Deserialize, Default)]
struct SummaryRuleConfig {
    #[serde(
        default,
        deserialize_with = "normalize_rules_config::deserialize_one_or_many"
    )]
    filenames: Vec<String>,
    #[serde(
        default,
        deserialize_with = "normalize_rules_config::deserialize_one_or_many"
    )]
    paths: Vec<String>,
}

/// Typed config for threshold-based rules (long-file, high-complexity, long-function).
/// Deserialized from `extra` fields on the `RuleOverride` via `rule_config()`.
#[derive(serde::Deserialize, Default)]
struct ThresholdConfig {
    threshold: Option<usize>,
}

/// Resolve pretty mode: enabled on TTY (or forced via --pretty), disabled by --compact.
fn resolve_pretty(pretty: bool, compact: bool) -> bool {
    use std::io::IsTerminal;
    !compact && (pretty || std::io::stdout().is_terminal())
}

/// A single error found when compiling a `.dl` rules file.
#[derive(serde::Serialize, schemars::JsonSchema)]
pub struct CompileError {
    /// 1-based line number in the source file (0 = unknown / not line-specific).
    pub line: usize,
    /// 1-based column number in the source file (0 = unknown).
    pub col: usize,
    /// Human-readable description of the error.
    pub message: String,
}

/// A single warning found when compiling a `.dl` rules file.
#[derive(serde::Serialize, schemars::JsonSchema)]
pub struct CompileWarning {
    /// 1-based line number in the source file (0 = unknown / not line-specific).
    pub line: usize,
    /// 1-based column number in the source file (0 = unknown).
    pub col: usize,
    /// Human-readable description of the warning.
    pub message: String,
}

/// Report returned by `normalize rules compile`.
#[derive(serde::Serialize, schemars::JsonSchema)]
pub struct RulesCompileReport {
    /// Path to the `.dl` file that was checked.
    pub path: String,
    /// `true` if no errors were found; `false` otherwise.
    pub valid: bool,
    /// Hard errors — the program cannot run correctly.
    pub errors: Vec<CompileError>,
    /// Warnings — the program will run but may not behave as expected.
    pub warnings: Vec<CompileWarning>,
    /// All relation names referenced in rule heads or bodies (sorted).
    pub relations_used: Vec<String>,
}

impl OutputFormatter for RulesCompileReport {
    fn format_text(&self) -> String {
        let mut out = String::new();
        for e in &self.errors {
            if e.line > 0 {
                out.push_str(&format!(
                    "{}:{}:{}: error: {}\n",
                    self.path, e.line, e.col, e.message
                ));
            } else {
                out.push_str(&format!("{}: error: {}\n", self.path, e.message));
            }
        }
        for w in &self.warnings {
            if w.line > 0 {
                out.push_str(&format!(
                    "{}:{}:{}: warning: {}\n",
                    self.path, w.line, w.col, w.message
                ));
            } else {
                out.push_str(&format!("{}: warning: {}\n", self.path, w.message));
            }
        }
        if self.valid {
            out.push_str(&format!(
                "{}: ok — {} relation{} used\n",
                self.path,
                self.relations_used.len(),
                if self.relations_used.len() == 1 {
                    ""
                } else {
                    "s"
                },
            ));
        }
        out
    }
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

impl RulesService {
    /// Generic display bridge that routes to `OutputFormatter::format_text()`.
    fn display_output<T: OutputFormatter>(&self, value: &T) -> String {
        if self.pretty.get() {
            value.format_pretty()
        } else {
            value.format_text()
        }
    }

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
    #[cli(display_with = "display_output")]
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
        let rule_type: RuleKind = r#type
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
    ///   normalize rules run --only "*.rs"      # only Rust files
    ///   normalize rules run --exclude "tests/" # skip test directories
    ///   normalize rules run --files src/main.rs src/lib.rs  # explicit file list
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
        #[param(help = "Only include files matching glob patterns")] only: Vec<String>,
        #[param(help = "Exclude files matching glob patterns")] exclude: Vec<String>,
        #[param(help = "Explicit file paths to check (bypasses file walker)")] files: Vec<String>,
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
        // Cap limit to prevent accidental OOM from huge values (e.g. usize::MAX).
        let limit = limit.unwrap_or(50).min(10_000);
        self.limit.set(limit);
        let config = load_rules_config(&effective_root);
        let rule_type: RuleKind = r#type
            .as_deref()
            .unwrap_or("all")
            .parse()
            .unwrap_or_default();
        let target_root = target
            .as_deref()
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| effective_root.clone());
        let project_root = effective_root.clone();

        // Resolve --files to absolute paths (relative to effective_root).
        let explicit_files: Option<Vec<std::path::PathBuf>> = if files.is_empty() {
            None
        } else {
            Some(
                files
                    .iter()
                    .map(|f| {
                        let p = std::path::PathBuf::from(f);
                        if p.is_absolute() {
                            p
                        } else {
                            effective_root.join(p)
                        }
                    })
                    .collect(),
            )
        };

        // Parse --only / --exclude into a PathFilter early so every engine can
        // skip non-matching files *before* doing expensive work (parsing, walking).
        let path_filter = normalize_rules_config::PathFilter::new(&only, &exclude);

        // --fix path: syntax-only, loop until no fixable issues remain
        if fix {
            let debug_flags = normalize_syntax_rules::DebugFlags::from_args(&debug);
            let fix_filter = path_filter.clone();
            tokio::task::spawn_blocking(move || {
                const MAX_FIX_ITERATIONS: usize = 100;
                let mut total_fixed = 0usize;
                let mut total_files = 0usize;
                let mut iterations = 0usize;
                loop {
                    if iterations >= MAX_FIX_ITERATIONS {
                        eprintln!(
                            "Warning: --fix stopped after {MAX_FIX_ITERATIONS} iterations; \
                             a rule fix may be generating output that still matches the same rule."
                        );
                        break;
                    }
                    iterations += 1;
                    let findings = run_syntax_rules(
                        &target_root,
                        &project_root,
                        rule.as_deref(),
                        tag.as_deref(),
                        None,
                        &config.rules,
                        &debug_flags,
                        explicit_files.as_deref(),
                        &fix_filter,
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

        let run_native = matches!(rule_type, RuleKind::All | RuleKind::Native);
        let rule_filter = rule.clone();

        // Syntax + fact + SARIF engines via run_rules_report() (blocking)
        let explicit_files_clone = explicit_files.clone();
        let syntax_filter = path_filter.clone();
        let mut report = tokio::task::spawn_blocking(move || {
            run_rules_report(
                &target_root,
                &project_root,
                rule.as_deref(),
                tag.as_deref(),
                &rule_type,
                &debug,
                &config,
                explicit_files_clone.as_deref(),
                &syntax_filter,
            )
        })
        .await
        .map_err(|e| format!("Task error: {e}"))?;

        // Native engine (missing-summary, stale-summary, check-refs, stale-docs, check-examples,
        // ratchet, budget, long-file, high-complexity, long-function)
        // runs in async context; included in All and Native engine types.
        // All checks are independent — run them in parallel.
        if run_native {
            let native_root = effective_root.clone();
            let native_config = load_rules_config(&native_root);
            let threshold = 10;
            let missing_summary_cfg: SummaryRuleConfig = native_config
                .rules
                .rules
                .get("missing-summary")
                .map(|r| r.rule_config())
                .unwrap_or_default();
            let missing_summary_filenames = missing_summary_cfg.filenames;
            let missing_summary_paths = missing_summary_cfg.paths;
            let stale_summary_cfg: SummaryRuleConfig = native_config
                .rules
                .rules
                .get("stale-summary")
                .map(|r| r.rule_config())
                .unwrap_or_default();
            let stale_summary_filenames = stale_summary_cfg.filenames;
            let stale_summary_paths = stale_summary_cfg.paths;

            // Helper: check if a native rule is enabled given config overrides and
            // the descriptor's default_enabled. A `--rule <id>` filter implicitly
            // enables the targeted rule so users can test disabled rules on demand.
            let is_native_enabled = |rule_id: &str| -> bool {
                if rule_filter.as_deref() == Some(rule_id) {
                    return true;
                }
                let desc = normalize_native_rules::NATIVE_RULES
                    .iter()
                    .find(|d| d.id == rule_id);
                let default = desc.map(|d| d.default_enabled).unwrap_or(true);
                native_config
                    .rules
                    .rules
                    .get(rule_id)
                    .and_then(|o| o.enabled)
                    .unwrap_or(default)
            };

            let run_long_file = is_native_enabled("long-file");
            let run_high_complexity = is_native_enabled("high-complexity");
            let run_long_function = is_native_enabled("long-function");

            let long_file_threshold: usize = native_config
                .rules
                .rules
                .get("long-file")
                .map(|r| r.rule_config::<ThresholdConfig>())
                .and_then(|c| c.threshold)
                .unwrap_or(500);
            let high_complexity_threshold: usize = native_config
                .rules
                .rules
                .get("high-complexity")
                .map(|r| r.rule_config::<ThresholdConfig>())
                .and_then(|c| c.threshold)
                .unwrap_or(20);
            let long_function_threshold: usize = native_config
                .rules
                .rules
                .get("long-function")
                .map(|r| r.rule_config::<ThresholdConfig>())
                .and_then(|c| c.threshold)
                .unwrap_or(100);

            let (
                missing_res,
                summary_res,
                stale_res,
                examples_res,
                refs_res,
                ratchet_res,
                budget_res,
            ) = tokio::join!(
                tokio::task::spawn_blocking({
                    let root = native_root.clone();
                    move || {
                        normalize_native_rules::build_missing_summary_report(
                            &root,
                            threshold,
                            &missing_summary_filenames,
                            &missing_summary_paths,
                        )
                    }
                }),
                tokio::task::spawn_blocking({
                    let root = native_root.clone();
                    move || {
                        normalize_native_rules::build_stale_summary_report(
                            &root,
                            threshold,
                            &stale_summary_filenames,
                            &stale_summary_paths,
                        )
                    }
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

            // Track how many issues existed before adding native results so
            // global_allow filtering only touches the newly added native issues.
            let native_start = report.issues.len();

            if let Ok(r) = missing_res {
                report.merge(r.into());
            }
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

            // When --only/--exclude is active but no --files given, merge the
            // path filter into explicit_files so advisory rules skip non-matching
            // files before parsing.  When --files *is* given, the filter refines
            // the explicit list.
            let effective_files: Option<Vec<std::path::PathBuf>> = if !path_filter.is_empty() {
                if let Some(ref ef) = explicit_files {
                    Some(
                        ef.iter()
                            .filter(|p| {
                                let rel = p.strip_prefix(&native_root).unwrap_or(p);
                                path_filter.matches_path(rel)
                            })
                            .cloned()
                            .collect(),
                    )
                } else {
                    // Walk once, filtering by PathFilter, and share the list
                    // across all advisory rules.
                    Some(
                        normalize_native_rules::walk::filtered_gitignore_walk(
                            &native_root,
                            &path_filter,
                        )
                        .filter(|e| e.path().is_file())
                        .map(|e| e.path().to_path_buf())
                        .collect(),
                    )
                }
            } else {
                explicit_files.clone()
            };

            // Advisory threshold rules (default disabled — only run when explicitly enabled).
            // These can be expensive (tree-sitter parsing), so we check enabled status first.
            let (long_file_res, high_complexity_res, long_function_res) = tokio::join!(
                async {
                    if !run_long_file {
                        return None;
                    }
                    let root = native_root.clone();
                    let threshold = long_file_threshold;
                    let ef = effective_files.clone();
                    tokio::task::spawn_blocking(move || {
                        normalize_native_rules::build_long_file_report(
                            &root,
                            threshold,
                            ef.as_deref(),
                        )
                    })
                    .await
                    .ok()
                },
                async {
                    if !run_high_complexity {
                        return None;
                    }
                    let root = native_root.clone();
                    let threshold = high_complexity_threshold;
                    let ef = effective_files.clone();
                    tokio::task::spawn_blocking(move || {
                        normalize_native_rules::build_high_complexity_report(
                            &root,
                            threshold,
                            ef.as_deref(),
                        )
                    })
                    .await
                    .ok()
                },
                async {
                    if !run_long_function {
                        return None;
                    }
                    let root = native_root.clone();
                    let threshold = long_function_threshold;
                    let ef = effective_files.clone();
                    tokio::task::spawn_blocking(move || {
                        normalize_native_rules::build_long_function_report(
                            &root,
                            threshold,
                            ef.as_deref(),
                        )
                    })
                    .await
                    .ok()
                },
            );

            if let Some(r) = long_file_res {
                report.merge(r);
            }
            if let Some(r) = high_complexity_res {
                report.merge(r);
            }
            if let Some(r) = long_function_res {
                report.merge(r);
            }

            // Apply global_allow patterns to native-rule issues (syntax/fact rules
            // apply global_allow during their own execution; native rules need it here).
            let global_allow: Vec<glob::Pattern> = native_config
                .rules
                .global_allow
                .iter()
                .filter_map(|s| glob::Pattern::new(s).ok())
                .collect();
            if !global_allow.is_empty() {
                let mut keep_idx = 0usize;
                report.issues.retain(|issue| {
                    let idx = keep_idx;
                    keep_idx += 1;
                    if idx < native_start {
                        // pre-existing syntax/fact issue — already filtered, keep it
                        return true;
                    }
                    !global_allow.iter().any(|p| p.matches(&issue.file))
                });
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

        // Safety-net: apply --only / --exclude path filters to all issues post-walk.
        // The pre-walk filter handles syntax and advisory native rules; this catches
        // engines that don't support pre-walk filtering (fact rules, non-advisory native).
        if !path_filter.is_empty() {
            report
                .issues
                .retain(|issue| path_filter.matches(issue.file.as_str()));
            let unique_files: std::collections::HashSet<&str> =
                report.issues.iter().map(|i| i.file.as_str()).collect();
            report.files_checked = unique_files.len();
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
    #[cli(display_with = "display_output")]
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
    #[cli(display_with = "display_output")]
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
    #[cli(display_with = "display_output")]
    pub fn show(
        &self,
        #[param(positional, help = "Rule ID to show")] id: String,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
        pretty: bool,
        compact: bool,
    ) -> Result<RuleInfoReport, String> {
        let effective_root = root
            .as_deref()
            .map(std::path::PathBuf::from)
            .map(Ok)
            .unwrap_or_else(std::env::current_dir)
            .map_err(|e| format!("Failed to get current directory: {e}"))?;
        self.resolve_format(pretty, compact);
        let config = load_rules_config(&effective_root);
        show_rule_structured(&effective_root, &id, &config)
    }

    /// List all tags and the rules they group
    ///
    /// Examples:
    ///   normalize rules tags                   # list all tags with rule counts
    #[cli(display_with = "display_output")]
    pub fn tags(
        &self,
        #[param(help = "Show only this specific tag")] tag: Option<String>,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
        pretty: bool,
        compact: bool,
    ) -> Result<RulesTagsReport, String> {
        let effective_root = root
            .as_deref()
            .map(std::path::PathBuf::from)
            .map(Ok)
            .unwrap_or_else(std::env::current_dir)
            .map_err(|e| format!("Failed to get current directory: {e}"))?;
        self.resolve_format(pretty, compact);
        let config = load_rules_config(&effective_root);
        list_tags_structured(&effective_root, tag.as_deref(), &config)
    }

    /// Add a rule from a URL
    ///
    /// Examples:
    ///   normalize rules add https://example.com/rule.scm   # import a rule from URL
    #[cli(display_with = "display_output")]
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
    #[cli(display_with = "display_output")]
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
    #[cli(display_with = "display_output")]
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
    #[cli(display_with = "display_output")]
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
    #[cli(display_with = "display_output")]
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
                type_filter: &RuleKind::All,
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

        Ok(report)
    }

    /// Validate and "compile" a Datalog rules file — check syntax and relation names
    ///
    /// Parses the `.dl` file, validates that all referenced relations are declared (or
    /// are built-in), and reports errors with file/line context.  Exits with status 1
    /// when errors are found so it can be used in CI pipelines.
    ///
    /// Examples:
    ///   normalize rules compile my-rule.dl       # check a single rule file
    ///   normalize rules compile .normalize/rules/arch.dl --json  # machine-readable output
    #[cli(display_with = "display_output")]
    pub fn compile(
        &self,
        #[param(positional, help = "Path to the .dl file to validate")] path: String,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
        pretty: bool,
        compact: bool,
    ) -> Result<RulesCompileReport, String> {
        use normalize_facts_rules_interpret::{compile_rules_source, parse_rule_content};

        self.resolve_format(pretty, compact);

        let effective_root = root
            .as_deref()
            .map(std::path::PathBuf::from)
            .map(Ok)
            .unwrap_or_else(std::env::current_dir)
            .map_err(|e| format!("Failed to get current directory: {e}"))?;

        let dl_path = if std::path::Path::new(&path).is_absolute() {
            std::path::PathBuf::from(&path)
        } else {
            effective_root.join(&path)
        };

        let content = std::fs::read_to_string(&dl_path)
            .map_err(|e| format!("Failed to read '{}': {e}", dl_path.display()))?;

        // Strip frontmatter to get the Datalog source body
        let default_id = dl_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("rule");
        let rule = parse_rule_content(&content, default_id, false);
        let source = rule.as_ref().map(|r| r.source.as_str()).unwrap_or(&content);

        let compile_result = compile_rules_source(source);

        let errors: Vec<CompileError> = compile_result
            .errors
            .into_iter()
            .map(|e| CompileError {
                line: e.line,
                col: e.col,
                message: e.message,
            })
            .collect();

        let warnings: Vec<CompileWarning> = compile_result
            .warnings
            .into_iter()
            .map(|w| CompileWarning {
                line: w.line,
                col: w.col,
                message: w.message,
            })
            .collect();

        let valid = errors.is_empty();

        let report = RulesCompileReport {
            path,
            valid,
            errors,
            warnings,
            relations_used: compile_result.relations_used,
        };

        if report.valid {
            Ok(report)
        } else {
            // Signal failure to the CLI via Err so the process exits with status 1.
            // The formatted output is passed as the error string so it appears on stderr.
            Err(self.display_output(&report))
        }
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
    let content = match std::fs::read_to_string(&project_path) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => {
            tracing::warn!("failed to read config at {:?}: {}", project_path, e);
            String::new()
        }
    };

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
        .map(|p| match std::fs::read_to_string(&p) {
            Ok(s) => s,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
            Err(e) => {
                tracing::warn!("failed to read config at {:?}: {}", p, e);
                String::new()
            }
        })
        .unwrap_or_default();

    let global: RulesOnlyConfig = toml::from_str(&global_content).unwrap_or_else(|e| {
        eprintln!("warning: failed to parse global rules config: {e}");
        eprintln!("  Rule overrides, severity settings, and allow patterns will not apply.");
        RulesOnlyConfig::default()
    });
    let project: RulesOnlyConfig = toml::from_str(&content).unwrap_or_else(|e| {
        eprintln!(
            "warning: failed to parse rules config at {}: {e}",
            project_path.display()
        );
        eprintln!("  Rule overrides, severity settings, and allow patterns will not apply.");
        RulesOnlyConfig::default()
    });

    let rule_tags = {
        let mut merged = global.rule_tags;
        merged.extend(project.rule_tags);
        merged
    };

    // Merge: start with global, overlay project on top.  Using normalize_core::Merge
    // ensures per-rule overrides from global config are preserved when the project
    // config only overrides a subset of rules.
    use normalize_core::Merge as _;
    let rules = global.rules.merge(project.rules);
    RulesRunConfig { rule_tags, rules }
}
