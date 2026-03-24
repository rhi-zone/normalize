//! CLI service for the budget system.
//!
//! Implements `normalize budget` subcommands via the server-less `#[cli]` pattern.

use crate::budget::{BudgetConfig, BudgetEntry, BudgetLimits, load_budget, save_budget};
use crate::error::BudgetError;
use crate::{DiffMetricFactory, default_diff_metrics};
use normalize_metrics::Aggregate;
use normalize_output::OutputFormatter;
use serde::Serialize;
use server_less::cli;
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// Report types
// ---------------------------------------------------------------------------

/// Result of `budget measure` — current diff stats for a path+metric.
#[derive(Debug, Clone, Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct MeasureReport {
    /// File or directory path being measured.
    pub path: String,
    /// Name of the metric being measured.
    pub metric: String,
    /// Aggregation function applied to the metric values (e.g. `sum`, `mean`).
    pub aggregate: String,
    #[serde(rename = "ref")]
    /// Git ref used as the base for the diff.
    pub base_ref: String,
    /// Total items added (in the aggregated result).
    pub added: f64,
    /// Total items removed (in the aggregated result).
    pub removed: f64,
    /// `added + removed` (total churn).
    pub total: f64,
    /// `added − removed` (net growth; negative means net shrinkage).
    pub net: f64,
    /// Number of diff items matched before aggregation.
    pub item_count: usize,
}

impl OutputFormatter for MeasureReport {
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
    /// File or directory path being checked.
    pub path: String,
    /// Name of the metric being checked.
    pub metric: String,
    /// Aggregation function applied to metric values.
    pub aggregate: Aggregate,
    #[serde(rename = "ref")]
    /// Git ref used as the base for the diff.
    pub base_ref: String,
    /// Total items added relative to the base ref.
    pub added: f64,
    /// Total items removed relative to the base ref.
    pub removed: f64,
    /// `added + removed` (total churn).
    pub total: f64,
    /// `added − removed` (net growth; negative means net shrinkage).
    pub net: f64,
    /// Human-readable descriptions of each limit that was exceeded.
    pub violations: Vec<String>,
}

/// Result of `budget check`.
#[derive(Debug, Clone, Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct CheckReport {
    pub entries: Vec<CheckEntry>,
    /// Total number of limit violations across all entries.
    pub violations: usize,
    /// Number of entries with no violations.
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
pub struct AddReport {
    /// File or directory path for the new budget entry.
    pub path: String,
    /// Name of the metric the entry tracks.
    pub metric: String,
    /// True if the entry was created; false if it already existed.
    pub added: bool,
}

impl OutputFormatter for AddReport {
    fn format_text(&self) -> String {
        if self.added {
            format!("added budget entry: {}  metric={}", self.path, self.metric)
        } else {
            format!(
                "budget entry already exists for {}/{}; use `budget update` to modify",
                self.path, self.metric
            )
        }
    }
}

/// Result of `budget update`.
#[derive(Debug, Clone, Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct UpdateReport {
    /// File or directory path of the budget entry being updated.
    pub path: String,
    /// Name of the metric for the entry being updated.
    pub metric: String,
    /// True if the entry was found and modified.
    pub updated: bool,
}

impl OutputFormatter for UpdateReport {
    fn format_text(&self) -> String {
        if self.updated {
            format!(
                "updated budget entry: {}  metric={}",
                self.path, self.metric
            )
        } else {
            "entry not found".to_string()
        }
    }
}

/// A single entry in `budget show`.
#[derive(Debug, Clone, Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct ShowEntry {
    /// File or directory path the entry applies to.
    pub path: String,
    /// Name of the metric the entry tracks.
    pub metric: String,
    /// Aggregation function applied to metric values.
    pub aggregate: String,
    #[serde(rename = "ref")]
    /// Git ref used as the base for the diff.
    pub base_ref: String,
    /// Budget limits configured for this entry.
    pub limits: BudgetLimits,
}

/// Result of `budget show`.
#[derive(Debug, Clone, Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct ShowReport {
    /// Budget entries matching the query.
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
            if let Some(v) = l.max_added {
                out.push_str(&format!("    max_added: {v}\n"));
            }
            if let Some(v) = l.max_removed {
                out.push_str(&format!("    max_removed: {v}\n"));
            }
            if let Some(v) = l.max_total {
                out.push_str(&format!("    max_total: {v}\n"));
            }
            if let Some(v) = l.max_net {
                out.push_str(&format!("    max_net: {v}\n"));
            }
        }
        out
    }
}

/// Result of `budget remove`.
#[derive(Debug, Clone, Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct RemoveReport {
    pub path: String,
    pub metric: String,
    /// True if the entry was found and removed.
    pub removed: bool,
}

impl OutputFormatter for RemoveReport {
    fn format_text(&self) -> String {
        if self.removed {
            format!(
                "removed budget entry: {}  metric={}",
                self.path, self.metric
            )
        } else {
            format!("no entry found for {}  metric={}", self.path, self.metric)
        }
    }
}

// ---------------------------------------------------------------------------
// Service
// ---------------------------------------------------------------------------

/// Budget sub-service: diff-based budget tracking.
pub struct BudgetService {
    pretty: std::cell::Cell<bool>,
    diff_factory: DiffMetricFactory,
}

impl BudgetService {
    /// Create a service using the default diff metric registry.
    pub fn new(pretty: bool) -> Self {
        Self {
            pretty: std::cell::Cell::new(pretty),
            diff_factory: default_diff_metrics,
        }
    }

    /// Create a service with a custom diff metric factory.
    pub fn with_factory(pretty: bool, factory: DiffMetricFactory) -> Self {
        Self {
            pretty: std::cell::Cell::new(pretty),
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

    fn display_measure(&self, r: &MeasureReport) -> String {
        r.format_text()
    }

    fn display_check(&self, r: &CheckReport) -> String {
        r.format_text()
    }

    fn display_add(&self, r: &AddReport) -> String {
        r.format_text()
    }

    fn display_update(&self, r: &UpdateReport) -> String {
        r.format_text()
    }

    fn display_show(&self, r: &ShowReport) -> String {
        r.format_text()
    }

    fn display_remove(&self, r: &RemoveReport) -> String {
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
        aggregate: Option<Aggregate>,
        pretty: bool,
        compact: bool,
    ) -> Result<MeasureReport, String> {
        let root = resolve_root(root)?;
        self.resolve_format(pretty, compact);
        let config = load_budget_config(&root);
        let base_ref = base_ref
            .as_deref()
            .unwrap_or_else(|| config.effective_ref())
            .to_string();
        let agg = aggregate.unwrap_or_else(|| config.effective_aggregate());
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
        #[param(short = 'a', help = "Aggregation strategy")] aggregate: Option<Aggregate>,
        #[param(help = "Maximum items added")] max_added: Option<f64>,
        #[param(help = "Maximum items removed")] max_removed: Option<f64>,
        #[param(help = "Maximum total churn (added + removed)")] max_total: Option<f64>,
        #[param(help = "Maximum net change (added - removed)")] max_net: Option<f64>,
        pretty: bool,
        compact: bool,
    ) -> Result<AddReport, String> {
        let root = resolve_root(root)?;
        self.resolve_format(pretty, compact);
        let config = load_budget_config(&root);
        let base_ref = base_ref
            .as_deref()
            .unwrap_or_else(|| config.effective_ref())
            .to_string();
        let agg = aggregate.unwrap_or_else(|| config.effective_aggregate());

        let limits = BudgetLimits {
            max_added,
            max_removed,
            max_total,
            max_net,
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
            return Ok(AddReport {
                path,
                metric,
                added: false,
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

        Ok(AddReport {
            path,
            metric,
            added: true,
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
        let root = resolve_root(root)?;
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
                aggregate: entry.aggregate,
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
    ) -> Result<UpdateReport, String> {
        let root = resolve_root(root)?;
        self.resolve_format(pretty, compact);
        let mut budget = load_budget(&root).map_err(|e| e.to_string())?;

        let entry = budget
            .entries
            .iter_mut()
            .find(|e| e.path == path && e.metric == metric);

        match entry {
            None => Ok(UpdateReport {
                path,
                metric,
                updated: false,
            }),
            Some(e) => {
                if max_added.is_some() {
                    e.limits.max_added = max_added;
                }
                if max_removed.is_some() {
                    e.limits.max_removed = max_removed;
                }
                if max_total.is_some() {
                    e.limits.max_total = max_total;
                }
                if max_net.is_some() {
                    e.limits.max_net = max_net;
                }
                save_budget(&root, &budget).map_err(|e| e.to_string())?;
                Ok(UpdateReport {
                    path,
                    metric,
                    updated: true,
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
        let root = resolve_root(root)?;
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
    ) -> Result<RemoveReport, String> {
        let root = resolve_root(root)?;
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

        Ok(RemoveReport {
            path,
            metric,
            removed,
        })
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn resolve_root(root: Option<String>) -> Result<PathBuf, String> {
    match root {
        Some(r) => Ok(PathBuf::from(r)),
        None => {
            std::env::current_dir().map_err(|e| format!("failed to get current directory: {e}"))
        }
    }
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
    match toml::from_str::<Wrapper>(&content) {
        Ok(w) => w.budget,
        Err(e) => {
            eprintln!("warning: failed to parse budget config: {e}");
            BudgetConfig::default()
        }
    }
}

/// Measure current diff stats for a path prefix and metric, filtered and aggregated.
pub(crate) fn do_measure(
    root: &Path,
    path: &str,
    metric: &str,
    base_ref: &str,
    agg: Aggregate,
    factory: &DiffMetricFactory,
) -> Result<MeasureReport, String> {
    let metrics = factory();
    let m = metrics.iter().find(|m| m.name() == metric).ok_or_else(|| {
        BudgetError::MetricNotFound {
            name: metric.to_string(),
        }
        .to_string()
    })?;

    let all = m.measure_diff(root, base_ref).map_err(|e| {
        BudgetError::MeasurementFailed {
            metric: metric.to_string(),
            path: root.display().to_string(),
            reason: e.to_string(),
        }
        .to_string()
    })?;

    // Filter items whose key starts with path prefix
    let path_prefix = path.trim_end_matches('/');
    let filtered: Vec<(f64, f64)> = all
        .into_iter()
        .filter(|item| {
            item.key == path_prefix
                || item.key.starts_with(&format!("{path_prefix}/"))
                || (path_prefix.ends_with('/') && item.key.starts_with(path_prefix))
        })
        .map(|item| (item.added, item.removed))
        .collect();

    // Warn if a configured path prefix matched nothing — this often indicates a typo.
    if filtered.is_empty() {
        eprintln!(
            "info: budget path prefix '{path}' matched no diff items for metric '{metric}' (ref={base_ref})"
        );
    }

    let item_count = filtered.len();

    let added_values: Vec<f64> = filtered.iter().map(|(a, _)| *a).collect();
    let removed_values: Vec<f64> = filtered.iter().map(|(_, r)| *r).collect();

    let added = normalize_metrics::compute_aggregate(added_values, agg).unwrap_or(0.0);
    let removed = normalize_metrics::compute_aggregate(removed_values, agg).unwrap_or(0.0);

    // Guard against NaN propagating into limit comparisons.
    if !added.is_finite() || !removed.is_finite() {
        return Err(format!(
            "metric '{}' at '{}': computed non-finite value (added={added}, removed={removed}); \
             check for NaN-producing aggregation inputs",
            metric,
            root.display()
        ));
    }

    let total = added + removed;
    let net = added - removed;

    Ok(MeasureReport {
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

fn path_matches(addr: &str, prefix: &str) -> bool {
    let canonical = prefix.trim_end_matches('/');
    addr == canonical || addr.starts_with(&format!("{canonical}/"))
}

fn filter_entries<'a>(
    entries: &'a [BudgetEntry],
    path: Option<&str>,
    metric: Option<&str>,
) -> Vec<&'a BudgetEntry> {
    entries
        .iter()
        .filter(|e| path.is_none_or(|p| path_matches(&e.path, p)))
        .filter(|e| metric.is_none_or(|m| e.metric == m))
        .collect()
}

fn check_limits(result: &MeasureReport, limits: &BudgetLimits) -> Vec<String> {
    let mut violations = Vec::new();
    if let Some(max) = limits.max_added
        && result.added > max
    {
        violations.push(format!(
            "added={:.0} exceeds max_added={max:.0}",
            result.added
        ));
    }
    if let Some(max) = limits.max_removed
        && result.removed > max
    {
        violations.push(format!(
            "removed={:.0} exceeds max_removed={max:.0}",
            result.removed
        ));
    }
    if let Some(max) = limits.max_total
        && result.total > max
    {
        violations.push(format!(
            "total={:.0} exceeds max_total={max:.0}",
            result.total
        ));
    }
    if let Some(max) = limits.max_net
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
pub fn build_budget_report(
    root: &Path,
    factory: &DiffMetricFactory,
) -> normalize_output::diagnostics::DiagnosticsReport {
    use normalize_output::diagnostics::{DiagnosticsReport, Issue, Severity};

    let budget = match load_budget(root) {
        Ok(b) => b,
        Err(e) => {
            // File-not-found returns default; only real errors reach here.
            return DiagnosticsReport {
                issues: vec![Issue {
                    file: root.to_string_lossy().to_string(),
                    line: None,
                    column: None,
                    end_line: None,
                    end_column: None,
                    rule_id: "budget/load".to_string(),
                    message: format!("budget: failed to load budget file: {e}"),
                    severity: Severity::Error,
                    source: "budget".into(),
                    related: vec![],
                    suggestion: None,
                }],
                files_checked: 0,
                sources_run: vec!["budget".into()],
                tool_errors: vec![],
            };
        }
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

// Keep the old name as an alias for backward compat with normalize-native-rules.
#[doc(hidden)]
pub use build_budget_report as build_budget_diagnostics;
