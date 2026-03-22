//! CLI service for the budget system.
//!
//! Implements `normalize budget` subcommands via the server-less `#[cli]` pattern.

use crate::budget::{BudgetConfig, BudgetEntry, BudgetLimits, load_budget, save_budget};
use crate::{DiffMetricFactory, default_diff_metrics};
use normalize_metrics::Aggregate;
use normalize_output::OutputFormatter;
use serde::Serialize;
use server_less::cli;
use std::cell::Cell;
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// Report types
// ---------------------------------------------------------------------------

/// Result of `budget measure` — current diff stats for a path+metric.
#[derive(Debug, Clone, Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct MeasureResult {
    pub path: String,
    pub metric: String,
    pub aggregate: String,
    #[serde(rename = "ref")]
    pub base_ref: String,
    pub added: f64,
    pub removed: f64,
    pub total: f64,
    pub net: f64,
    pub item_count: usize,
}

impl OutputFormatter for MeasureResult {
    fn format_text(&self) -> String {
        format!(
            "{}  metric={} aggregate={} ref={}\n  added={:.0} removed={:.0} total={:.0} net={:.0} ({} items)",
            self.path,
            self.metric,
            self.aggregate,
            self.base_ref,
            self.added,
            self.removed,
            self.total,
            self.net,
            self.item_count,
        )
    }
}

/// A single entry in a `budget check` report.
#[derive(Debug, Clone, Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct CheckEntry {
    pub path: String,
    pub metric: String,
    pub aggregate: String,
    #[serde(rename = "ref")]
    pub base_ref: String,
    pub added: f64,
    pub removed: f64,
    pub total: f64,
    pub net: f64,
    pub violations: Vec<String>,
}

/// Result of `budget check`.
#[derive(Debug, Clone, Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct CheckReport {
    pub entries: Vec<CheckEntry>,
    pub violations: usize,
    pub ok: usize,
}

impl OutputFormatter for CheckReport {
    fn format_text(&self) -> String {
        if self.entries.is_empty() {
            return "budget: no entries".to_string();
        }
        let mut out = String::new();
        for e in &self.entries {
            if e.violations.is_empty() {
                out.push_str(&format!("  ok    {}  metric={}\n", e.path, e.metric));
            } else {
                out.push_str(&format!("  FAIL  {}  metric={}\n", e.path, e.metric));
                for v in &e.violations {
                    out.push_str(&format!("        {v}\n"));
                }
            }
        }
        out.push_str(&format!("{} violation(s), {} ok", self.violations, self.ok));
        out
    }
}

/// Result of `budget add`.
#[derive(Debug, Clone, Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct AddResult {
    pub path: String,
    pub metric: String,
    pub added: bool,
    pub message: String,
}

impl OutputFormatter for AddResult {
    fn format_text(&self) -> String {
        self.message.clone()
    }
}

/// Result of `budget update`.
#[derive(Debug, Clone, Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct UpdateResult {
    pub path: String,
    pub metric: String,
    pub updated: bool,
    pub message: String,
}

impl OutputFormatter for UpdateResult {
    fn format_text(&self) -> String {
        self.message.clone()
    }
}

/// A single entry in `budget show`.
#[derive(Debug, Clone, Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct ShowEntry {
    pub path: String,
    pub metric: String,
    pub aggregate: String,
    #[serde(rename = "ref")]
    pub base_ref: String,
    pub limits: BudgetLimits,
}

/// Result of `budget show`.
#[derive(Debug, Clone, Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct ShowReport {
    pub entries: Vec<ShowEntry>,
}

impl OutputFormatter for ShowReport {
    fn format_text(&self) -> String {
        if self.entries.is_empty() {
            return "budget: no matching entries".to_string();
        }
        let mut out = String::new();
        for e in &self.entries {
            out.push_str(&format!(
                "  {}  metric={}  aggregate={}  ref={}\n",
                e.path, e.metric, e.aggregate, e.base_ref
            ));
            let l = &e.limits;
            if let Some(v) = l.added {
                out.push_str(&format!("    max_added: {v}\n"));
            }
            if let Some(v) = l.removed {
                out.push_str(&format!("    max_removed: {v}\n"));
            }
            if let Some(v) = l.total {
                out.push_str(&format!("    max_total: {v}\n"));
            }
            if let Some(v) = l.net {
                out.push_str(&format!("    max_net: {v}\n"));
            }
        }
        out
    }
}

/// Result of `budget remove`.
#[derive(Debug, Clone, Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct RemoveResult {
    pub path: String,
    pub metric: String,
    pub removed: bool,
    pub message: String,
}

impl OutputFormatter for RemoveResult {
    fn format_text(&self) -> String {
        self.message.clone()
    }
}

// ---------------------------------------------------------------------------
// Service
// ---------------------------------------------------------------------------

/// Budget sub-service: diff-based budget tracking.
pub struct BudgetService {
    pretty: Cell<bool>,
    diff_factory: DiffMetricFactory,
}

impl BudgetService {
    pub fn new(pretty: &Cell<bool>) -> Self {
        Self {
            pretty: Cell::new(pretty.get()),
            diff_factory: default_diff_metrics,
        }
    }

    pub fn with_factory(pretty: &Cell<bool>, factory: DiffMetricFactory) -> Self {
        Self {
            pretty: Cell::new(pretty.get()),
            diff_factory: factory,
        }
    }

    fn resolve_format(&self, pretty: bool, compact: bool) {
        if pretty {
            self.pretty.set(true);
        } else if compact {
            self.pretty.set(false);
        }
    }

    fn display_measure(&self, r: &MeasureResult) -> String {
        r.format_text()
    }

    fn display_check(&self, r: &CheckReport) -> String {
        r.format_text()
    }

    fn display_add(&self, r: &AddResult) -> String {
        r.format_text()
    }

    fn display_update(&self, r: &UpdateResult) -> String {
        r.format_text()
    }

    fn display_show(&self, r: &ShowReport) -> String {
        r.format_text()
    }

    fn display_remove(&self, r: &RemoveResult) -> String {
        r.format_text()
    }
}

#[cli(global = [
    pretty = "Human-friendly output with colors and formatting",
    compact = "Compact output without colors (overrides TTY detection)",
])]
impl BudgetService {
    /// Compute current diff stats for a path+metric. No side effects.
    #[cli(display_with = "display_measure")]
    #[allow(clippy::too_many_arguments)]
    pub fn measure(
        &self,
        #[param(positional, help = "Path to measure (file, directory, or prefix)")] path: String,
        #[param(
            short = 'm',
            help = "Diff metric (lines|functions|classes|modules|todos|complexity-delta|dependencies)"
        )]
        metric: String,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
        #[param(help = "Git ref to diff against")] base_ref: Option<String>,
        #[param(
            short = 'a',
            help = "Aggregation strategy (mean|median|max|min|sum|count)"
        )]
        aggregate: Option<String>,
        pretty: bool,
        compact: bool,
    ) -> Result<MeasureResult, String> {
        let root = resolve_root(root);
        self.resolve_format(pretty, compact);
        let config = load_budget_config(&root);
        let base_ref = base_ref.unwrap_or_else(|| config.effective_ref());
        let agg = parse_aggregate(aggregate, &config)?;
        do_measure(&root, &path, &metric, &base_ref, agg, &self.diff_factory)
    }

    /// Add a budget entry. At least one limit required. Errors if entry already exists.
    #[cli(display_with = "display_add")]
    #[allow(clippy::too_many_arguments)]
    pub fn add(
        &self,
        #[param(positional, help = "Path to track")] path: String,
        #[param(short = 'm', help = "Diff metric to track")] metric: String,
        #[param(short = 'r', help = "Root directory")] root: Option<String>,
        #[param(help = "Git ref to diff against")] base_ref: Option<String>,
        #[param(short = 'a', help = "Aggregation strategy")] aggregate: Option<String>,
        #[param(help = "Maximum items added")] max_added: Option<f64>,
        #[param(help = "Maximum items removed")] max_removed: Option<f64>,
        #[param(help = "Maximum total churn (added + removed)")] max_total: Option<f64>,
        #[param(help = "Maximum net change (added - removed)")] max_net: Option<f64>,
        pretty: bool,
        compact: bool,
    ) -> Result<AddResult, String> {
        let root = resolve_root(root);
        self.resolve_format(pretty, compact);
        let config = load_budget_config(&root);
        let base_ref = base_ref.unwrap_or_else(|| config.effective_ref());
        let agg = parse_aggregate(aggregate, &config)?;

        let limits = BudgetLimits {
            added: max_added,
            removed: max_removed,
            total: max_total,
            net: max_net,
        };

        if limits.is_empty() {
            return Err(
                "at least one limit (--max-added, --max-removed, --max-total, --max-net) is required"
                    .to_string(),
            );
        }

        let mut budget = load_budget(&root).map_err(|e| e.to_string())?;

        if budget
            .entries
            .iter()
            .any(|e| e.path == path && e.metric == metric)
        {
            return Ok(AddResult {
                path: path.clone(),
                metric: metric.clone(),
                added: false,
                message: format!(
                    "budget entry already exists for {path}/{metric}; use `budget update` to modify"
                ),
            });
        }

        let entry = BudgetEntry {
            path: path.clone(),
            metric: metric.clone(),
            aggregate: agg,
            base_ref: base_ref.clone(),
            limits,
        };
        budget.entries.push(entry);
        save_budget(&root, &budget).map_err(|e| e.to_string())?;

        Ok(AddResult {
            path: path.clone(),
            metric: metric.clone(),
            added: true,
            message: format!("added budget entry: {path}  metric={metric}  ref={base_ref}"),
        })
    }

    /// Check all matching entries against current diff. Exits non-zero if any limit exceeded.
    #[cli(display_with = "display_check")]
    pub fn check(
        &self,
        #[param(positional, help = "Filter by path prefix")] path: Option<String>,
        #[param(short = 'm', help = "Filter by metric name")] metric: Option<String>,
        #[param(short = 'r', help = "Root directory")] root: Option<String>,
        pretty: bool,
        compact: bool,
    ) -> Result<CheckReport, String> {
        let root = resolve_root(root);
        self.resolve_format(pretty, compact);
        let budget = load_budget(&root).map_err(|e| e.to_string())?;
        let entries = filter_entries(&budget.entries, path.as_deref(), metric.as_deref());

        let mut check_entries = Vec::new();
        let mut violations = 0usize;
        let mut ok = 0usize;

        for entry in entries {
            let result = match do_measure(
                &root,
                &entry.path,
                &entry.metric,
                &entry.base_ref,
                entry.aggregate,
                &self.diff_factory,
            ) {
                Ok(r) => r,
                Err(e) => {
                    eprintln!(
                        "warning: could not measure {} ({}): {e}",
                        entry.path, entry.metric
                    );
                    continue;
                }
            };

            let entry_violations = check_limits(&result, &entry.limits);
            if entry_violations.is_empty() {
                ok += 1;
            } else {
                violations += entry_violations.len();
            }

            check_entries.push(CheckEntry {
                path: entry.path.clone(),
                metric: entry.metric.clone(),
                aggregate: entry.aggregate.to_string(),
                base_ref: entry.base_ref.clone(),
                added: result.added,
                removed: result.removed,
                total: result.total,
                net: result.net,
                violations: entry_violations,
            });
        }

        let report = CheckReport {
            entries: check_entries,
            violations,
            ok,
        };

        if report.violations > 0 {
            let detail = report.format_text();
            return Err(format!(
                "{detail}\n{} budget violation(s) found",
                report.violations
            ));
        }

        Ok(report)
    }

    /// Update limits on an existing budget entry.
    #[cli(display_with = "display_update")]
    #[allow(clippy::too_many_arguments)]
    pub fn update(
        &self,
        #[param(positional, help = "Path of the entry to update")] path: String,
        #[param(short = 'm', help = "Metric of the entry to update")] metric: String,
        #[param(short = 'r', help = "Root directory")] root: Option<String>,
        #[param(help = "New maximum items added")] max_added: Option<f64>,
        #[param(help = "New maximum items removed")] max_removed: Option<f64>,
        #[param(help = "New maximum total churn")] max_total: Option<f64>,
        #[param(help = "New maximum net change")] max_net: Option<f64>,
        pretty: bool,
        compact: bool,
    ) -> Result<UpdateResult, String> {
        let root = resolve_root(root);
        self.resolve_format(pretty, compact);
        let mut budget = load_budget(&root).map_err(|e| e.to_string())?;

        let entry = budget
            .entries
            .iter_mut()
            .find(|e| e.path == path && e.metric == metric);

        match entry {
            None => Ok(UpdateResult {
                path,
                metric,
                updated: false,
                message: "entry not found".to_string(),
            }),
            Some(e) => {
                if max_added.is_some() {
                    e.limits.added = max_added;
                }
                if max_removed.is_some() {
                    e.limits.removed = max_removed;
                }
                if max_total.is_some() {
                    e.limits.total = max_total;
                }
                if max_net.is_some() {
                    e.limits.net = max_net;
                }
                save_budget(&root, &budget).map_err(|e| e.to_string())?;
                Ok(UpdateResult {
                    path: path.clone(),
                    metric: metric.clone(),
                    updated: true,
                    message: format!("updated budget entry: {path}  metric={metric}"),
                })
            }
        }
    }

    /// Display matching budget entries with current diff stats.
    #[cli(display_with = "display_show")]
    pub fn show(
        &self,
        #[param(positional, help = "Filter by path prefix")] path: Option<String>,
        #[param(short = 'm', help = "Filter by metric name")] metric: Option<String>,
        #[param(short = 'r', help = "Root directory")] root: Option<String>,
        pretty: bool,
        compact: bool,
    ) -> Result<ShowReport, String> {
        let root = resolve_root(root);
        self.resolve_format(pretty, compact);
        let budget = load_budget(&root).map_err(|e| e.to_string())?;
        let entries = filter_entries(&budget.entries, path.as_deref(), metric.as_deref());

        let show_entries = entries
            .into_iter()
            .map(|e| ShowEntry {
                path: e.path.clone(),
                metric: e.metric.clone(),
                aggregate: e.aggregate.to_string(),
                base_ref: e.base_ref.clone(),
                limits: e.limits.clone(),
            })
            .collect();

        Ok(ShowReport {
            entries: show_entries,
        })
    }

    /// Remove a budget entry.
    #[cli(display_with = "display_remove")]
    pub fn remove(
        &self,
        #[param(positional, help = "Path of the entry to remove")] path: String,
        #[param(short = 'm', help = "Metric of the entry to remove")] metric: String,
        #[param(short = 'r', help = "Root directory")] root: Option<String>,
        pretty: bool,
        compact: bool,
    ) -> Result<RemoveResult, String> {
        let root = resolve_root(root);
        self.resolve_format(pretty, compact);
        let mut budget = load_budget(&root).map_err(|e| e.to_string())?;

        let len_before = budget.entries.len();
        budget
            .entries
            .retain(|e| !(e.path == path && e.metric == metric));
        let removed = budget.entries.len() < len_before;

        if removed {
            save_budget(&root, &budget).map_err(|e| e.to_string())?;
        }

        let message = if removed {
            format!("removed budget entry: {path}  metric={metric}")
        } else {
            format!("no entry found for {path}  metric={metric}")
        };

        Ok(RemoveResult {
            path,
            metric,
            removed,
            message,
        })
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn resolve_root(root: Option<String>) -> PathBuf {
    root.map(PathBuf::from)
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
}

fn load_budget_config(root: &Path) -> BudgetConfig {
    let path = root.join(".normalize").join("config.toml");
    if !path.exists() {
        return BudgetConfig::default();
    }
    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => return BudgetConfig::default(),
    };
    #[derive(serde::Deserialize, Default)]
    struct Wrapper {
        #[serde(default)]
        budget: BudgetConfig,
    }
    let wrapper: Wrapper = toml::from_str(&content).unwrap_or_default();
    wrapper.budget
}

fn parse_aggregate(agg: Option<String>, config: &BudgetConfig) -> Result<Aggregate, String> {
    match agg {
        Some(s) => s.parse::<Aggregate>(),
        None => Ok(config.effective_aggregate()),
    }
}

pub(crate) fn do_measure(
    root: &Path,
    path: &str,
    metric: &str,
    base_ref: &str,
    agg: Aggregate,
    factory: &DiffMetricFactory,
) -> Result<MeasureResult, String> {
    let metrics = factory();
    let m = metrics
        .iter()
        .find(|m| m.name() == metric)
        .ok_or_else(|| format!("unknown diff metric '{metric}'"))?;

    let all = m.measure_diff(root, base_ref).map_err(|e| e.to_string())?;

    // Filter items whose key starts with path prefix
    let path_prefix = path.trim_end_matches('/');
    let filtered: Vec<(f64, f64)> = all
        .into_iter()
        .filter(|(addr, _, _)| {
            addr == path_prefix
                || addr.starts_with(&format!("{path_prefix}/"))
                || (path_prefix.ends_with('/') && addr.starts_with(path_prefix))
        })
        .map(|(_, added, removed)| (added, removed))
        .collect();

    let item_count = filtered.len();

    let mut added_values: Vec<f64> = filtered.iter().map(|(a, _)| *a).collect();
    let mut removed_values: Vec<f64> = filtered.iter().map(|(_, r)| *r).collect();

    let added = normalize_metrics::aggregate(&mut added_values, agg).unwrap_or(0.0);
    let removed = normalize_metrics::aggregate(&mut removed_values, agg).unwrap_or(0.0);
    let total = added + removed;
    let net = added - removed;

    Ok(MeasureResult {
        path: path.to_string(),
        metric: metric.to_string(),
        aggregate: agg.to_string(),
        base_ref: base_ref.to_string(),
        added,
        removed,
        total,
        net,
        item_count,
    })
}

fn filter_entries<'a>(
    entries: &'a [BudgetEntry],
    path: Option<&str>,
    metric: Option<&str>,
) -> Vec<&'a BudgetEntry> {
    entries
        .iter()
        .filter(|e| path.is_none_or(|p| e.path.starts_with(p)))
        .filter(|e| metric.is_none_or(|m| e.metric == m))
        .collect()
}

fn check_limits(result: &MeasureResult, limits: &BudgetLimits) -> Vec<String> {
    let mut violations = Vec::new();
    if let Some(max) = limits.added
        && result.added > max
    {
        violations.push(format!(
            "added={:.0} exceeds max_added={max:.0}",
            result.added
        ));
    }
    if let Some(max) = limits.removed
        && result.removed > max
    {
        violations.push(format!(
            "removed={:.0} exceeds max_removed={max:.0}",
            result.removed
        ));
    }
    if let Some(max) = limits.total
        && result.total > max
    {
        violations.push(format!(
            "total={:.0} exceeds max_total={max:.0}",
            result.total
        ));
    }
    if let Some(max) = limits.net
        && result.net > max
    {
        violations.push(format!("net={:.0} exceeds max_net={max:.0}", result.net));
    }
    violations
}

// ---------------------------------------------------------------------------
// Native rules integration
// ---------------------------------------------------------------------------

/// Build a DiagnosticsReport from budget check for use in `normalize rules run`.
pub fn build_budget_diagnostics(
    root: &Path,
    factory: &DiffMetricFactory,
) -> normalize_output::diagnostics::DiagnosticsReport {
    use normalize_output::diagnostics::{DiagnosticsReport, Issue, Severity};

    let budget = match load_budget(root) {
        Ok(b) => b,
        Err(_) => return DiagnosticsReport::new(),
    };

    if budget.entries.is_empty() {
        return DiagnosticsReport::new();
    }

    let mut issues = Vec::new();
    let entry_count = budget.entries.len();

    for entry in &budget.entries {
        let result = match do_measure(
            root,
            &entry.path,
            &entry.metric,
            &entry.base_ref,
            entry.aggregate,
            factory,
        ) {
            Ok(r) => r,
            Err(e) => {
                eprintln!(
                    "warning: budget measure failed for {} ({}): {e}",
                    entry.path, entry.metric
                );
                continue;
            }
        };

        let violations = check_limits(&result, &entry.limits);
        for v in violations {
            issues.push(Issue {
                file: entry.path.clone(),
                line: None,
                column: None,
                end_line: None,
                end_column: None,
                rule_id: format!("budget/{}", entry.metric),
                message: format!("{}: {v}", entry.path),
                severity: Severity::Error,
                source: "budget".into(),
                related: vec![],
                suggestion: Some(format!(
                    "run `normalize budget update {} --metric {}` to raise the limit",
                    entry.path, entry.metric
                )),
            });
        }
    }

    DiagnosticsReport {
        issues,
        files_checked: entry_count,
        sources_run: vec!["budget".into()],
        tool_errors: vec![],
    }
}
