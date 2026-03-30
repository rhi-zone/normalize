//! CLI service for the ratchet system.
//!
//! Implements `normalize ratchet` subcommands via the server-less `#[cli]` pattern.

use crate::baseline::{Aggregate, BaselineEntry, BaselineFile, RatchetConfig};
use crate::error::RatchetError;
use crate::git_ops;
use crate::{MetricFactory, default_metrics};
use normalize_metrics::filter_by_prefix;
use normalize_output::OutputFormatter;
use serde::Serialize;
use server_less::cli;
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// Report types
// ---------------------------------------------------------------------------

/// Result of a single measurement run.
///
/// Holds the current measured value for one metric at one path. This is the
/// output of `ratchet measure` and represents a snapshot of the current working
/// tree — it does not compare against any baseline.
#[derive(Debug, Clone, Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct MeasureReport {
    /// Relative path (or symbol address) that was measured.
    pub path: String,
    /// Name of the metric.
    pub metric: String,
    /// Aggregation strategy applied.
    pub aggregate: Aggregate,
    /// Aggregated metric value.
    pub value: f64,
    /// Number of individual items that contributed to the aggregate.
    pub item_count: usize,
}

impl OutputFormatter for MeasureReport {
    fn format_text(&self) -> String {
        format!(
            "{}  metric={} aggregate={} value={:.4} ({} items)",
            self.path, self.metric, self.aggregate, self.value, self.item_count
        )
    }
}

/// Result of `ratchet check`.
///
/// Compares current measured values against a pinned baseline. Each entry
/// records whether the metric regressed, improved, or stayed unchanged relative
/// to the baseline. The summary counts (`regressions`, `improvements`,
/// `unchanged`) reflect totals across all checked entries.
#[derive(Debug, Clone, Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct CheckReport {
    /// All checked entries with their current vs baseline values.
    pub entries: Vec<CheckEntry>,
    /// Number of entries that regressed.
    pub regressions: usize,
    /// Number of entries that improved.
    pub improvements: usize,
    /// Number of entries that were unchanged.
    pub unchanged: usize,
}

/// A single entry in a [`CheckReport`].
#[derive(Debug, Clone, Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct CheckEntry {
    /// Path (or symbol address) this entry tracks.
    pub path: String,
    /// Metric name.
    pub metric: String,
    /// Aggregation strategy.
    pub aggregate: Aggregate,
    /// Pinned baseline value.
    pub baseline: f64,
    /// Current measured value.
    pub current: f64,
    /// `current - baseline`.
    pub delta: f64,
    /// Whether this entry regressed, improved, or stayed the same.
    pub status: CheckStatus,
}

/// Classification of a check result relative to the baseline.
#[derive(
    Debug, Clone, Copy, Serialize, serde::Deserialize, schemars::JsonSchema, PartialEq, Eq,
)]
#[serde(rename_all = "lowercase")]
pub enum CheckStatus {
    /// The metric got worse (higher when higher-is-worse, or lower when lower-is-worse).
    Regression,
    /// The metric improved.
    Improvement,
    /// The metric did not change meaningfully.
    Unchanged,
}

impl std::fmt::Display for CheckStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CheckStatus::Regression => f.write_str("REGRESSION"),
            CheckStatus::Improvement => f.write_str("ok (improved)"),
            CheckStatus::Unchanged => f.write_str("ok"),
        }
    }
}

impl OutputFormatter for CheckReport {
    fn format_text(&self) -> String {
        let mut lines = Vec::new();
        lines.push(format!(
            "Ratchet check: {} regressions, {} improvements, {} unchanged",
            self.regressions, self.improvements, self.unchanged
        ));
        lines.push(String::new());
        if self.entries.is_empty() {
            lines.push("No entries checked.".to_string());
        } else {
            for e in &self.entries {
                let delta_str = if e.delta >= 0.0 {
                    format!("+{:.4}", e.delta)
                } else {
                    format!("{:.4}", e.delta)
                };
                lines.push(format!(
                    "  [{status}] {path} ({metric}/{agg}): {baseline:.4} → {current:.4} ({delta})",
                    status = e.status,
                    path = e.path,
                    metric = e.metric,
                    agg = e.aggregate,
                    baseline = e.baseline,
                    current = e.current,
                    delta = delta_str,
                ));
            }
        }
        lines.join("\n")
    }
}

/// Result of `ratchet update`.
#[derive(Debug, Clone, Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct UpdateReport {
    /// Entries whose baseline was written.
    pub updated: Vec<UpdateEntry>,
    /// Entries that were not updated (no improvement, or value unchanged).
    pub skipped: Vec<UpdateEntry>,
}

/// A single entry in an [`UpdateReport`].
#[derive(Debug, Clone, Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct UpdateEntry {
    /// File or directory path the baseline entry applies to.
    pub path: String,
    /// Name of the metric the entry tracks.
    pub metric: String,
    /// Baseline value before this update.
    pub old_value: f64,
    /// Baseline value after this update.
    pub new_value: f64,
    /// Why this entry was updated or skipped.
    pub reason: UpdateReason,
}

/// Reason an entry was updated or skipped during `ratchet update`.
#[derive(Debug, Clone, Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum UpdateReason {
    /// Updated because `--force` was passed.
    Forced,
    /// Updated because the metric improved.
    Improved,
    /// Skipped because the metric did not improve.
    NoImprovement,
}

impl std::fmt::Display for UpdateReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UpdateReason::Forced => f.write_str("forced"),
            UpdateReason::Improved => f.write_str("improved"),
            UpdateReason::NoImprovement => f.write_str("no improvement"),
        }
    }
}

impl OutputFormatter for UpdateReport {
    fn format_text(&self) -> String {
        let mut lines = Vec::new();
        lines.push(format!(
            "Ratchet update: {} updated, {} skipped",
            self.updated.len(),
            self.skipped.len()
        ));
        for e in &self.updated {
            lines.push(format!(
                "  updated {} ({}/{}): {:.4} → {:.4}",
                e.path, e.metric, e.reason, e.old_value, e.new_value
            ));
        }
        for e in &self.skipped {
            lines.push(format!(
                "  skipped {} ({}/{}): {:.4}",
                e.path, e.metric, e.reason, e.old_value
            ));
        }
        lines.join("\n")
    }
}

/// Result of `ratchet show`.
#[derive(Debug, Clone, Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct ShowReport {
    /// Baseline entries matching the query.
    pub entries: Vec<ShowEntry>,
}

/// A single entry in a [`ShowReport`].
#[derive(Debug, Clone, Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct ShowEntry {
    /// File or directory path the baseline entry applies to.
    pub path: String,
    /// Name of the metric the entry tracks.
    pub metric: String,
    /// Aggregation function applied to metric values.
    pub aggregate: Aggregate,
    /// Recorded baseline value for this path/metric/aggregate combination.
    pub value: f64,
}

impl OutputFormatter for ShowReport {
    fn format_text(&self) -> String {
        if self.entries.is_empty() {
            return "No entries found.".to_string();
        }
        let mut lines = Vec::new();
        lines.push(format!("{} entries:", self.entries.len()));
        for e in &self.entries {
            lines.push(format!(
                "  {} ({}/{}) = {:.4}",
                e.path, e.metric, e.aggregate, e.value
            ));
        }
        lines.join("\n")
    }
}

/// Result of `ratchet add`.
#[derive(Debug, Clone, Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct AddReport {
    /// File or directory path the new baseline entry applies to.
    pub path: String,
    /// Name of the metric the entry tracks.
    pub metric: String,
    /// Aggregation function applied to metric values.
    pub aggregate: Aggregate,
    /// Baseline value recorded for this entry.
    pub value: f64,
    /// Number of individual measurement items that were aggregated.
    pub item_count: usize,
}

impl OutputFormatter for AddReport {
    fn format_text(&self) -> String {
        format!(
            "Added baseline: {} ({}/{}) = {:.4} ({} items)",
            self.path, self.metric, self.aggregate, self.value, self.item_count
        )
    }
}

/// Result of `ratchet remove`.
#[derive(Debug, Clone, Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct RemoveReport {
    /// File or directory path the baseline entry applies to.
    pub path: String,
    /// Name of the metric the entry tracks.
    pub metric: String,
    /// Whether an entry was actually found and removed.
    pub removed: bool,
}

impl OutputFormatter for RemoveReport {
    fn format_text(&self) -> String {
        if self.removed {
            format!("Removed baseline entry: {} ({})", self.path, self.metric)
        } else {
            format!(
                "No baseline entry found for {} ({})",
                self.path, self.metric
            )
        }
    }
}

// ---------------------------------------------------------------------------
// Service
// ---------------------------------------------------------------------------

/// CLI service implementing `normalize ratchet` subcommands.
pub struct RatchetService {
    pretty: std::cell::Cell<bool>,
    metric_factory: MetricFactory,
}

impl RatchetService {
    /// Create a service using the default metric registry.
    pub fn new(pretty: bool) -> Self {
        Self {
            pretty: std::cell::Cell::new(pretty),
            metric_factory: default_metrics,
        }
    }

    /// Create a service with a custom metric factory, for testing or alternative metric sets.
    pub fn with_factory(pretty: bool, factory: MetricFactory) -> Self {
        Self {
            pretty: std::cell::Cell::new(pretty),
            metric_factory: factory,
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

    fn display_update(&self, r: &UpdateReport) -> String {
        r.format_text()
    }

    fn display_show(&self, r: &ShowReport) -> String {
        r.format_text()
    }

    fn display_add(&self, r: &AddReport) -> String {
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
impl RatchetService {
    /// Compute and display the current value for a path+metric. No side effects.
    #[cli(display_with = "display_measure")]
    #[allow(clippy::too_many_arguments)]
    pub fn measure(
        &self,
        #[param(positional, help = "Path to measure (file, directory, or symbol)")] path: String,
        #[param(
            short = 'm',
            help = "Metric to measure (complexity|call-complexity|line-count|function-count|class-count|comment-line-count)"
        )]
        metric: String,
        #[param(
            short = 'a',
            help = "Aggregation strategy (mean|median|max|min|sum|count)"
        )]
        aggregate: Option<Aggregate>,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
        #[param(help = "Compute diff against this git ref (measure delta vs this ref)")]
        diff_ref: Option<String>,
        pretty: bool,
        compact: bool,
    ) -> Result<MeasureReport, String> {
        let root_path = resolve_root(root)?;
        self.resolve_format(pretty, compact);
        let config = load_ratchet_config(&root_path);
        let agg: Aggregate = aggregate.unwrap_or_else(|| config.effective_aggregate(&metric));

        if let Some(ref base_ref) = diff_ref {
            return measure_at_ref(
                &root_path,
                base_ref,
                &path,
                &metric,
                agg,
                &self.metric_factory,
            );
        }

        measure(&root_path, &path, &metric, agg, &self.metric_factory)
    }

    /// Measure and pin as a new baseline entry. Errors if entry already exists.
    #[cli(display_with = "display_add")]
    pub fn add(
        &self,
        #[param(positional, help = "Path to track")] path: String,
        #[param(
            short = 'm',
            help = "Metric to track (complexity|call-complexity|line-count|function-count|class-count|comment-line-count)"
        )]
        metric: String,
        #[param(short = 'a', help = "Aggregation strategy")] aggregate: Option<Aggregate>,
        #[param(short = 'r', help = "Root directory")] root: Option<String>,
        pretty: bool,
        compact: bool,
    ) -> Result<AddReport, String> {
        let root_path = resolve_root(root)?;
        self.resolve_format(pretty, compact);
        let config = load_ratchet_config(&root_path);
        let agg: Aggregate = aggregate.unwrap_or_else(|| config.effective_aggregate(&metric));

        let mut baseline = BaselineFile::load(&root_path)
            .map_err(|e| e.to_string())?
            .unwrap_or_default();

        // Check duplicate
        if baseline
            .entries
            .iter()
            .any(|e| e.path == path && e.metric == metric)
        {
            return Err(format!(
                "baseline entry already exists for {path} ({metric}); use `ratchet update` to change it"
            ));
        }

        let result = measure(&root_path, &path, &metric, agg, &self.metric_factory)?;

        baseline.entries.push(BaselineEntry {
            path: path.clone(),
            metric: metric.clone(),
            aggregate: agg,
            value: result.value,
        });
        baseline.save(&root_path).map_err(|e| e.to_string())?;

        Ok(AddReport {
            path,
            metric,
            aggregate: agg,
            value: result.value,
            item_count: result.item_count,
        })
    }

    /// Compare current values to baseline entries.
    ///
    /// Without --baseline-ref: reads .normalize/ratchet.json.
    /// With --baseline-ref <ref>: measures baseline at that git ref, compares to current working tree.
    ///
    /// Exits non-zero if regressions found.
    #[cli(display_with = "display_check")]
    #[allow(clippy::too_many_arguments)]
    pub fn check(
        &self,
        #[param(positional, help = "Filter by path prefix")] path: Option<String>,
        #[param(short = 'm', help = "Filter by metric name")] metric: Option<String>,
        #[param(
            help = "Substitute this git ref as the baseline instead of the stored ratchet.json baseline"
        )]
        baseline_ref: Option<String>,
        #[param(short = 'a', help = "Aggregation strategy (used with --baseline-ref)")]
        aggregate: Option<Aggregate>,
        #[param(short = 'r', help = "Root directory")] root: Option<String>,
        pretty: bool,
        compact: bool,
    ) -> Result<CheckReport, String> {
        let root_path = resolve_root(root)?;
        self.resolve_format(pretty, compact);

        if let Some(ref base_ref) = baseline_ref {
            let config = load_ratchet_config(&root_path);
            let agg = aggregate.unwrap_or_else(|| {
                metric
                    .as_deref()
                    .map(|m| config.effective_aggregate(m))
                    .unwrap_or(Aggregate::Mean)
            });
            return check_against_ref(
                &root_path,
                base_ref,
                path.as_deref(),
                metric.as_deref(),
                agg,
                &self.metric_factory,
            );
        }

        let baseline = BaselineFile::load(&root_path)
            .map_err(|e| e.to_string())?
            .unwrap_or_default();
        let entries = filter_entries(&baseline.entries, path.as_deref(), metric.as_deref());

        let report = build_check_report(&root_path, entries, &self.metric_factory)?;

        if report.regressions > 0 {
            let detail = report.format_text();
            return Err(format!(
                "{detail}\n{} regression(s) found",
                report.regressions
            ));
        }

        Ok(report)
    }

    /// Re-measure matching entries and write new values.
    ///
    /// Without --force: only lowers values (true ratchet behaviour).
    /// With --force: also raises values.
    #[cli(display_with = "display_update")]
    pub fn update(
        &self,
        #[param(positional, help = "Filter by path prefix")] path: Option<String>,
        #[param(short = 'm', help = "Filter by metric")] metric: Option<String>,
        #[param(help = "Also raise values (not just lower them)")] force: bool,
        #[param(short = 'r', help = "Root directory")] root: Option<String>,
        pretty: bool,
        compact: bool,
    ) -> Result<UpdateReport, String> {
        let root_path = resolve_root(root)?;
        self.resolve_format(pretty, compact);

        let mut baseline = BaselineFile::load(&root_path)
            .map_err(|e| e.to_string())?
            .unwrap_or_default();
        let matching_indices: Vec<usize> = baseline
            .entries
            .iter()
            .enumerate()
            .filter(|(_, e)| {
                path.as_deref().is_none_or(|p| path_matches(&e.path, p))
                    && metric.as_deref().is_none_or(|m| e.metric == m)
            })
            .map(|(i, _)| i)
            .collect();

        let mut updated = Vec::new();
        let mut skipped = Vec::new();

        for idx in matching_indices {
            let entry = &baseline.entries[idx];
            let result = match measure(
                &root_path,
                &entry.path,
                &entry.metric,
                entry.aggregate,
                &self.metric_factory,
            ) {
                Ok(r) => r,
                Err(e) => {
                    tracing::warn!("could not measure {} ({}): {e}", entry.path, entry.metric);
                    continue;
                }
            };

            let old_value = entry.value;
            let new_value = result.value;
            let higher_is_worse = metric_higher_is_worse(&entry.metric, &self.metric_factory);

            let should_update = if force {
                (new_value - old_value).abs() > f64::EPSILON
            } else {
                // Only update if it would lower the ratchet (improve)
                if higher_is_worse {
                    new_value < old_value
                } else {
                    new_value > old_value
                }
            };

            if should_update {
                updated.push(UpdateEntry {
                    path: entry.path.clone(),
                    metric: entry.metric.clone(),
                    old_value,
                    new_value,
                    reason: if force {
                        UpdateReason::Forced
                    } else {
                        UpdateReason::Improved
                    },
                });
                baseline.entries[idx].value = new_value;
            } else {
                skipped.push(UpdateEntry {
                    path: entry.path.clone(),
                    metric: entry.metric.clone(),
                    old_value,
                    new_value,
                    reason: UpdateReason::NoImprovement,
                });
            }
        }

        if !updated.is_empty() {
            baseline.save(&root_path).map_err(|e| e.to_string())?;
        }

        Ok(UpdateReport { updated, skipped })
    }

    /// Display matching baseline entries.
    #[cli(display_with = "display_show")]
    pub fn show(
        &self,
        #[param(positional, help = "Filter by path prefix")] path: Option<String>,
        #[param(short = 'm', help = "Filter by metric")] metric: Option<String>,
        #[param(short = 'r', help = "Root directory")] root: Option<String>,
        pretty: bool,
        compact: bool,
    ) -> Result<ShowReport, String> {
        let root_path = resolve_root(root)?;
        self.resolve_format(pretty, compact);
        let baseline = BaselineFile::load(&root_path)
            .map_err(|e| e.to_string())?
            .unwrap_or_default();
        let entries: Vec<ShowEntry> =
            filter_entries(&baseline.entries, path.as_deref(), metric.as_deref())
                .into_iter()
                .map(|e| ShowEntry {
                    path: e.path.clone(),
                    metric: e.metric.clone(),
                    aggregate: e.aggregate,
                    value: e.value,
                })
                .collect();
        Ok(ShowReport { entries })
    }

    /// Remove a baseline entry.
    #[cli(display_with = "display_remove")]
    pub fn remove(
        &self,
        #[param(positional, help = "Path of the entry to remove")] path: String,
        #[param(short = 'm', help = "Metric of the entry to remove")] metric: String,
        #[param(short = 'r', help = "Root directory")] root: Option<String>,
        pretty: bool,
        compact: bool,
    ) -> Result<RemoveReport, String> {
        let root_path = resolve_root(root)?;
        self.resolve_format(pretty, compact);
        let mut baseline = BaselineFile::load(&root_path)
            .map_err(|e| e.to_string())?
            .unwrap_or_default();
        let before = baseline.entries.len();
        baseline
            .entries
            .retain(|e| !(e.path == path && e.metric == metric));
        let removed = baseline.entries.len() < before;
        if removed {
            baseline.save(&root_path).map_err(|e| e.to_string())?;
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
        None => std::env::current_dir().map_err(|e| format!("failed to get cwd: {e}")),
    }
}

fn load_ratchet_config(root: &Path) -> RatchetConfig {
    // Try to load from .normalize/config.toml
    let config_path = root.join(".normalize").join("config.toml");
    if let Ok(content) = std::fs::read_to_string(&config_path)
        && let Ok(toml_val) = toml::from_str::<toml::Value>(&content)
        && let Some(ratchet_section) = toml_val.get("ratchet")
        && let Ok(cfg) = toml::from_str::<RatchetConfig>(&ratchet_section.to_string())
    {
        return cfg;
    }
    RatchetConfig::default()
}

/// True if `addr` matches path prefix `prefix`, respecting path boundaries.
fn path_matches(addr: &str, prefix: &str) -> bool {
    let canonical = prefix.trim_end_matches('/');
    addr == canonical
        || addr.starts_with(&format!("{canonical}/"))
        || (prefix.ends_with('/') && addr.starts_with(prefix))
}

/// Collect and aggregate all metric values for the given metric name, filter by path prefix.
pub fn measure(
    root: &Path,
    path: &str,
    metric: &str,
    agg: Aggregate,
    factory: &MetricFactory,
) -> Result<MeasureReport, String> {
    let metrics = factory(root);
    let m = metrics.iter().find(|m| m.name() == metric).ok_or_else(|| {
        RatchetError::MetricNotFound {
            name: metric.to_string(),
        }
        .to_string()
    })?;

    let all = m.measure_all(root).map_err(|e| {
        RatchetError::MeasurementFailed {
            metric: metric.to_string(),
            path: root.display().to_string(),
            reason: e.to_string(),
        }
        .to_string()
    })?;

    let values: Vec<f64> = filter_by_prefix(&all, path).map(|p| p.value).collect();

    let item_count = values.len();

    match crate::baseline::compute_aggregate(values, agg) {
        Some(value) => Ok(MeasureReport {
            path: path.to_string(),
            metric: metric.to_string(),
            aggregate: agg,
            value,
            item_count,
        }),
        None => Err(format!(
            "no items matched path '{path}' for metric '{metric}'"
        )),
    }
}

/// Deprecated alias for [`measure`].
#[deprecated(since = "0.2.0", note = "use measure")]
#[inline]
pub fn do_measure(
    root: &Path,
    path: &str,
    metric: &str,
    agg: Aggregate,
    factory: &MetricFactory,
) -> Result<MeasureReport, String> {
    measure(root, path, metric, agg, factory)
}

/// Filter baseline entries by optional path prefix and metric name.
///
/// Both `path` and `metric` are optional; passing `None` means "no filter".
/// Path matching respects path boundaries (e.g. `"src"` matches `"src/foo"` but not `"srcs/bar"`).
fn filter_entries<'a>(
    entries: &'a [BaselineEntry],
    path: Option<&str>,
    metric: Option<&str>,
) -> Vec<&'a BaselineEntry> {
    entries
        .iter()
        .filter(|e| path.is_none_or(|p| path_matches(&e.path, p)))
        .filter(|e| metric.is_none_or(|m| e.metric == m))
        .collect()
}

/// Measure all `entries` against the current working tree and produce a [`CheckReport`].
///
/// Each entry is re-measured; entries whose metric cannot be measured are skipped with a warning.
/// The report counts regressions, improvements, and unchanged entries, and classifies each
/// [`CheckEntry`] according to whether higher-or-lower is worse for that metric.
fn build_check_report(
    root: &Path,
    entries: Vec<&BaselineEntry>,
    factory: &MetricFactory,
) -> Result<CheckReport, String> {
    let mut check_entries = Vec::new();
    let mut regressions = 0usize;
    let mut improvements = 0usize;
    let mut unchanged = 0usize;

    for entry in entries {
        let result = match measure(root, &entry.path, &entry.metric, entry.aggregate, factory) {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!("could not measure {} at {}: {e}", entry.metric, entry.path);
                continue;
            }
        };

        let current = result.value;
        let baseline = entry.value;
        let delta = current - baseline;

        // Guard against NaN/infinity deltas — skip the entry rather than misclassifying.
        if !delta.is_finite() {
            tracing::warn!(
                "non-finite delta for {} at {}: {delta}",
                entry.metric,
                entry.path
            );
            continue;
        }

        let metrics = factory(root);
        let higher_is_worse = metrics
            .iter()
            .find(|m| m.name() == entry.metric)
            .map(|m| m.higher_is_worse())
            .unwrap_or(true);

        let status = if delta.abs() < 1e-10 {
            CheckStatus::Unchanged
        } else if (higher_is_worse && delta > 0.0) || (!higher_is_worse && delta < 0.0) {
            CheckStatus::Regression
        } else {
            CheckStatus::Improvement
        };

        match status {
            CheckStatus::Regression => regressions += 1,
            CheckStatus::Improvement => improvements += 1,
            CheckStatus::Unchanged => unchanged += 1,
        }

        check_entries.push(CheckEntry {
            path: entry.path.clone(),
            metric: entry.metric.clone(),
            aggregate: entry.aggregate,
            baseline,
            current,
            delta,
            status,
        });
    }

    Ok(CheckReport {
        entries: check_entries,
        regressions,
        improvements,
        unchanged,
    })
}

/// Materialise all blobs at `base_ref` into a temporary directory and return its path.
///
/// The caller is responsible for cleaning up via the returned [`tempfile::TempDir`].
/// Uses gix to read blobs directly from the object store — no `git` binary required.
fn checkout_ref_to_tempdir(root: &Path, base_ref: &str) -> Result<tempfile::TempDir, String> {
    let tmp = tempfile::tempdir().map_err(|e| format!("failed to create temp dir: {e}"))?;
    let tmp_path = tmp.path().to_owned();

    let repo = git_ops::open_repo(root).map_err(|e| e.to_string())?;

    git_ops::walk_tree_at_ref(root, base_ref, |rel_path, blob_id| {
        let Some(content) = git_ops::read_blob_text(&repo, blob_id) else {
            return;
        };
        let dest = tmp_path.join(rel_path);
        if let Some(parent) = dest.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(&dest, content.as_bytes());
    })
    .map_err(|e| format!("failed to walk git tree at '{base_ref}': {e}"))?;

    Ok(tmp)
}

/// Check against a historical git ref.
///
/// Reads all file blobs at `base_ref` from the git object store via gix (no `git` binary),
/// writes them to a temporary directory, runs metric collection there, then compares against
/// the current working tree.
fn check_against_ref(
    root: &Path,
    base_ref: &str,
    path_filter: Option<&str>,
    metric_filter: Option<&str>,
    aggregate: Aggregate,
    factory: &MetricFactory,
) -> Result<CheckReport, String> {
    // Materialise ref into a temp dir — dropped (and deleted) at end of scope.
    let tmp = checkout_ref_to_tempdir(root, base_ref)?;
    let ref_path = tmp.path();

    // Measure baseline at ref
    let baseline_measurements = {
        let metrics = factory(ref_path);
        let metric_names: Vec<&str> = if let Some(m) = metric_filter {
            metrics
                .iter()
                .filter(|x| x.name() == m)
                .map(|x| x.name())
                .collect()
        } else {
            metrics.iter().map(|x| x.name()).collect()
        };

        let mut measurements: Vec<(String, String, f64)> = Vec::new(); // (path, metric, value)
        for metric_name in metric_names {
            let m = metrics
                .iter()
                .find(|x| x.name() == metric_name)
                .ok_or_else(|| format!("metric '{}' not found", metric_name))?;
            let all = match m.measure_all(ref_path) {
                Ok(v) => v,
                Err(e) => {
                    tracing::warn!(
                        "could not measure {} at base ref {}: {}",
                        metric_name,
                        base_ref,
                        e
                    );
                    continue;
                }
            };
            for (addr, val) in all {
                if path_filter.is_none_or(|p| path_matches(&addr, p)) {
                    measurements.push((addr, metric_name.to_string(), val));
                }
            }
        }
        measurements
    };

    // Measure current
    let current_metrics = factory(root);
    let mut check_entries = Vec::new();
    let mut regressions = 0;
    let mut improvements = 0;
    let mut unchanged = 0;

    // Group baseline measurements by (metric)
    let mut baseline_by_metric: std::collections::HashMap<String, Vec<(String, f64)>> =
        std::collections::HashMap::new();
    for (addr, metric_name, val) in baseline_measurements {
        baseline_by_metric
            .entry(metric_name)
            .or_default()
            .push((addr, val));
    }

    for (metric_name, baseline_items) in &baseline_by_metric {
        let m = match current_metrics
            .iter()
            .find(|x| x.name() == metric_name.as_str())
        {
            Some(m) => m,
            None => continue,
        };
        let current_all = match m.measure_all(root) {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!("could not measure {} at current tree: {e}", metric_name);
                continue;
            }
        };
        let current_map: std::collections::HashMap<&str, f64> =
            current_all.iter().map(|(k, v)| (k.as_str(), *v)).collect();

        for (addr, baseline_val) in baseline_items {
            let current_val = match current_map.get(addr.as_str()) {
                Some(&v) => v,
                None => continue,
            };
            let delta = current_val - baseline_val;
            if !delta.is_finite() {
                continue;
            }
            let higher_is_worse = m.higher_is_worse();
            let status = if delta.abs() < 1e-10 {
                CheckStatus::Unchanged
            } else if (higher_is_worse && delta > 0.0) || (!higher_is_worse && delta < 0.0) {
                CheckStatus::Regression
            } else {
                CheckStatus::Improvement
            };
            match status {
                CheckStatus::Regression => regressions += 1,
                CheckStatus::Improvement => improvements += 1,
                CheckStatus::Unchanged => unchanged += 1,
            }
            check_entries.push(CheckEntry {
                path: addr.clone(),
                metric: metric_name.clone(),
                aggregate,
                baseline: *baseline_val,
                current: current_val,
                delta,
                status,
            });
        }
    }

    let report = CheckReport {
        entries: check_entries,
        regressions,
        improvements,
        unchanged,
    };

    if report.regressions > 0 {
        let detail = report.format_text();
        return Err(format!(
            "{detail}\n{} regression(s) found",
            report.regressions
        ));
    }

    Ok(report)
}

/// Measure at a historical git ref.
///
/// Reads all file blobs at `base_ref` from the git object store via gix (no `git` binary),
/// writes them to a temporary directory, then runs metric collection there.
fn measure_at_ref(
    root: &Path,
    base_ref: &str,
    path: &str,
    metric: &str,
    agg: Aggregate,
    factory: &MetricFactory,
) -> Result<MeasureReport, String> {
    let tmp = checkout_ref_to_tempdir(root, base_ref)?;
    measure(tmp.path(), path, metric, agg, factory)
}

fn metric_higher_is_worse(metric_name: &str, factory: &MetricFactory) -> bool {
    // Use a dummy path since we only need to check properties
    let root = std::path::Path::new(".");
    let metrics = factory(root);
    metrics
        .iter()
        .find(|m| m.name() == metric_name)
        .map(|m| m.higher_is_worse())
        .unwrap_or(true)
}

// ---------------------------------------------------------------------------
// Ratchet native rule support
// ---------------------------------------------------------------------------

/// Build a DiagnosticsReport from ratchet check for use in `normalize rules run`.
pub fn build_ratchet_report(
    root: &Path,
    factory: &MetricFactory,
) -> normalize_output::diagnostics::DiagnosticsReport {
    use normalize_output::diagnostics::{DiagnosticsReport, Issue, Severity};

    let baseline = match BaselineFile::load(root) {
        Ok(Some(b)) => b,
        Ok(None) => {
            // Ratchet not initialised — return empty report silently.
            return DiagnosticsReport::new();
        }
        Err(e) => {
            // Real IO/parse error — surface it as a diagnostic.
            return DiagnosticsReport {
                issues: vec![Issue {
                    file: root.to_string_lossy().into_owned(),
                    line: None,
                    column: None,
                    end_line: None,
                    end_column: None,
                    rule_id: "ratchet/load-error".into(),
                    message: format!("ratchet: failed to load baseline: {e}"),
                    severity: Severity::Error,
                    source: "ratchet".into(),
                    related: vec![],
                    suggestion: None,
                }],
                files_checked: 0,
                sources_run: vec!["ratchet".into()],
                tool_errors: vec![],
                daemon_cached: false,
            };
        }
    };

    if baseline.entries.is_empty() {
        return DiagnosticsReport::new();
    }

    let entries: Vec<&BaselineEntry> = baseline.entries.iter().collect();
    let report = match build_check_report(root, entries, factory) {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!("ratchet: check failed: {e}");
            return DiagnosticsReport::new();
        }
    };

    let issues: Vec<Issue> = report
        .entries
        .into_iter()
        .filter(|e| e.status == CheckStatus::Regression)
        .map(|e| {
            let rule_id = format!("ratchet/{}", e.metric);
            let message = format!(
                "{} ({}): {:.4} → {:.4} (delta {:.4})",
                e.path, e.aggregate, e.baseline, e.current, e.delta
            );
            Issue {
                file: e.path.clone(),
                line: None,
                column: None,
                end_line: None,
                end_column: None,
                rule_id,
                message,
                severity: Severity::Error,
                source: "ratchet".into(),
                related: vec![],
                suggestion: Some(format!(
                    "run `normalize ratchet update {} --metric {}` to accept new baseline",
                    e.path, e.metric
                )),
            }
        })
        .collect();

    DiagnosticsReport {
        issues,
        files_checked: baseline.entries.len(),
        sources_run: vec!["ratchet".into()],
        tool_errors: vec![],
        daemon_cached: false,
    }
}

/// Build a [`DiagnosticsReport`](normalize_output::diagnostics::DiagnosticsReport) from the
/// ratchet baseline check. Delegates to [`build_ratchet_report`] and returns its inner
/// `DiagnosticsReport` directly. Kept for compatibility with `normalize-native-rules`.
#[deprecated(since = "0.2.0", note = "use build_ratchet_report")]
#[inline]
pub fn build_ratchet_diagnostics(
    root: &Path,
    factory: &MetricFactory,
) -> normalize_output::diagnostics::DiagnosticsReport {
    build_ratchet_report(root, factory)
}
