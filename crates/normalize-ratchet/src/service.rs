//! CLI service for the ratchet system.
//!
//! Implements `normalize ratchet` subcommands via the server-less `#[cli]` pattern.

use crate::baseline::{Aggregate, BaselineEntry, RatchetConfig, load_baseline, save_baseline};
use crate::{MetricFactory, default_metrics};
use normalize_output::OutputFormatter;
use serde::Serialize;
use server_less::cli;
use std::cell::Cell;
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// Report types
// ---------------------------------------------------------------------------

/// Result of a single measurement.
#[derive(Debug, Clone, Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct MeasureResult {
    pub path: String,
    pub metric: String,
    pub aggregate: String,
    pub value: f64,
    pub item_count: usize,
}

impl OutputFormatter for MeasureResult {
    fn format_text(&self) -> String {
        format!(
            "{}  metric={} aggregate={} value={:.4} ({} items)",
            self.path, self.metric, self.aggregate, self.value, self.item_count
        )
    }
}

/// Result of `ratchet check`.
#[derive(Debug, Clone, Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct CheckReport {
    pub entries: Vec<CheckEntry>,
    pub regressions: usize,
    pub improvements: usize,
    pub unchanged: usize,
}

#[derive(Debug, Clone, Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct CheckEntry {
    pub path: String,
    pub metric: String,
    pub aggregate: String,
    pub baseline: f64,
    pub current: f64,
    pub delta: f64,
    pub status: CheckStatus,
}

#[derive(
    Debug, Clone, Copy, Serialize, serde::Deserialize, schemars::JsonSchema, PartialEq, Eq,
)]
#[serde(rename_all = "lowercase")]
pub enum CheckStatus {
    Regression,
    Improvement,
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
    pub updated: Vec<UpdateEntry>,
    pub skipped: Vec<UpdateEntry>,
}

#[derive(Debug, Clone, Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct UpdateEntry {
    pub path: String,
    pub metric: String,
    pub old_value: f64,
    pub new_value: f64,
    pub reason: String,
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
                "  skipped {} ({}/{}): {:.4} ({})",
                e.path, e.metric, e.reason, e.old_value, e.reason
            ));
        }
        lines.join("\n")
    }
}

/// Result of `ratchet show`.
#[derive(Debug, Clone, Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct ShowReport {
    pub entries: Vec<ShowEntry>,
}

#[derive(Debug, Clone, Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct ShowEntry {
    pub path: String,
    pub metric: String,
    pub aggregate: String,
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
pub struct AddResult {
    pub path: String,
    pub metric: String,
    pub aggregate: String,
    pub value: f64,
    pub item_count: usize,
}

impl OutputFormatter for AddResult {
    fn format_text(&self) -> String {
        format!(
            "Added baseline: {} ({}/{}) = {:.4} ({} items)",
            self.path, self.metric, self.aggregate, self.value, self.item_count
        )
    }
}

/// Result of `ratchet remove`.
#[derive(Debug, Clone, Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct RemoveResult {
    pub path: String,
    pub metric: String,
    pub removed: bool,
}

impl OutputFormatter for RemoveResult {
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

/// Ratchet sub-service: metric regression tracking.
pub struct RatchetService {
    pretty: Cell<bool>,
    metric_factory: MetricFactory,
}

impl RatchetService {
    pub fn new(pretty: &Cell<bool>) -> Self {
        Self {
            pretty: Cell::new(pretty.get()),
            metric_factory: default_metrics,
        }
    }

    pub fn with_factory(pretty: &Cell<bool>, factory: MetricFactory) -> Self {
        Self {
            pretty: Cell::new(pretty.get()),
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

    fn display_measure(&self, r: &MeasureResult) -> String {
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

    fn display_add(&self, r: &AddResult) -> String {
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
        aggregate: Option<String>,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
        #[param(help = "Show delta vs this git ref")] base: Option<String>,
        pretty: bool,
        compact: bool,
    ) -> Result<MeasureResult, String> {
        let root_path = resolve_root(root);
        self.resolve_format(pretty, compact);
        let config = load_ratchet_config(&root_path);
        let agg: Aggregate = parse_aggregate(aggregate, &config, &metric)?;

        if let Some(ref base_ref) = base {
            return measure_at_ref(
                &root_path,
                base_ref,
                &path,
                &metric,
                agg,
                &self.metric_factory,
            );
        }

        do_measure(&root_path, &path, &metric, agg, &self.metric_factory)
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
        #[param(short = 'a', help = "Aggregation strategy")] aggregate: Option<String>,
        #[param(short = 'r', help = "Root directory")] root: Option<String>,
        pretty: bool,
        compact: bool,
    ) -> Result<AddResult, String> {
        let root_path = resolve_root(root);
        self.resolve_format(pretty, compact);
        let config = load_ratchet_config(&root_path);
        let agg: Aggregate = parse_aggregate(aggregate, &config, &metric)?;

        let mut baseline = load_baseline(&root_path).map_err(|e| e.to_string())?;

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

        let result = do_measure(&root_path, &path, &metric, agg, &self.metric_factory)?;

        baseline.entries.push(BaselineEntry {
            path: path.clone(),
            metric: metric.clone(),
            aggregate: agg,
            value: result.value,
        });
        save_baseline(&root_path, &baseline).map_err(|e| e.to_string())?;

        Ok(AddResult {
            path,
            metric,
            aggregate: agg.to_string(),
            value: result.value,
            item_count: result.item_count,
        })
    }

    /// Compare current values to baseline entries.
    ///
    /// Without --base: reads .normalize/ratchet.json.
    /// With --base <ref>: measures baseline at that git ref, compares to current working tree.
    ///
    /// Exits non-zero if regressions found.
    #[cli(display_with = "display_check")]
    pub fn check(
        &self,
        #[param(positional, help = "Filter by path prefix")] path: Option<String>,
        #[param(short = 'm', help = "Filter by metric name")] metric: Option<String>,
        #[param(help = "Compare to this git ref instead of stored baseline")] base: Option<String>,
        #[param(short = 'r', help = "Root directory")] root: Option<String>,
        pretty: bool,
        compact: bool,
    ) -> Result<CheckReport, String> {
        let root_path = resolve_root(root);
        self.resolve_format(pretty, compact);

        if let Some(ref base_ref) = base {
            return check_against_ref(
                &root_path,
                base_ref,
                path.as_deref(),
                metric.as_deref(),
                &self.metric_factory,
            );
        }

        let baseline = load_baseline(&root_path).map_err(|e| e.to_string())?;
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
        let root_path = resolve_root(root);
        self.resolve_format(pretty, compact);

        let mut baseline = load_baseline(&root_path).map_err(|e| e.to_string())?;
        let matching_indices: Vec<usize> = baseline
            .entries
            .iter()
            .enumerate()
            .filter(|(_, e)| {
                path.as_deref().is_none_or(|p| e.path.starts_with(p))
                    && metric.as_deref().is_none_or(|m| e.metric == m)
            })
            .map(|(i, _)| i)
            .collect();

        let mut updated = Vec::new();
        let mut skipped = Vec::new();

        for idx in matching_indices {
            let entry = &baseline.entries[idx];
            let result = match do_measure(
                &root_path,
                &entry.path,
                &entry.metric,
                entry.aggregate,
                &self.metric_factory,
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
                        "forced".to_string()
                    } else {
                        "improved".to_string()
                    },
                });
                baseline.entries[idx].value = new_value;
            } else {
                skipped.push(UpdateEntry {
                    path: entry.path.clone(),
                    metric: entry.metric.clone(),
                    old_value,
                    new_value,
                    reason: "no improvement".to_string(),
                });
            }
        }

        if !updated.is_empty() {
            save_baseline(&root_path, &baseline).map_err(|e| e.to_string())?;
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
        let root_path = resolve_root(root);
        self.resolve_format(pretty, compact);
        let baseline = load_baseline(&root_path).map_err(|e| e.to_string())?;
        let entries: Vec<ShowEntry> =
            filter_entries(&baseline.entries, path.as_deref(), metric.as_deref())
                .into_iter()
                .map(|e| ShowEntry {
                    path: e.path.clone(),
                    metric: e.metric.clone(),
                    aggregate: e.aggregate.to_string(),
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
    ) -> Result<RemoveResult, String> {
        let root_path = resolve_root(root);
        self.resolve_format(pretty, compact);
        let mut baseline = load_baseline(&root_path).map_err(|e| e.to_string())?;
        let before = baseline.entries.len();
        baseline
            .entries
            .retain(|e| !(e.path == path && e.metric == metric));
        let removed = baseline.entries.len() < before;
        if removed {
            save_baseline(&root_path, &baseline).map_err(|e| e.to_string())?;
        }
        Ok(RemoveResult {
            path,
            metric,
            removed,
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

fn parse_aggregate(
    agg: Option<String>,
    config: &RatchetConfig,
    metric: &str,
) -> Result<Aggregate, String> {
    match agg {
        Some(s) => s.parse::<Aggregate>(),
        None => Ok(config.effective_aggregate(metric)),
    }
}

/// Perform a measurement: collect all metric values, filter by path prefix, aggregate.
pub fn do_measure(
    root: &Path,
    path: &str,
    metric: &str,
    agg: Aggregate,
    factory: &MetricFactory,
) -> Result<MeasureResult, String> {
    let metrics = factory(root);
    let m = metrics
        .iter()
        .find(|m| m.name() == metric)
        .ok_or_else(|| format!("unknown metric '{metric}'"))?;

    let all = m.measure_all(root).map_err(|e| e.to_string())?;

    // Filter items whose address starts with the path prefix
    let path_prefix = path.trim_end_matches('/');
    let mut values: Vec<f64> = all
        .into_iter()
        .filter(|(addr, _)| {
            addr == path_prefix
                || addr.starts_with(&format!("{path_prefix}/"))
                || addr.starts_with(path_prefix) && path_prefix.ends_with('/')
        })
        .map(|(_, v)| v)
        .collect();

    let item_count = values.len();

    match crate::baseline::aggregate(&mut values, agg) {
        Some(value) => Ok(MeasureResult {
            path: path.to_string(),
            metric: metric.to_string(),
            aggregate: agg.to_string(),
            value,
            item_count,
        }),
        None => Err(format!(
            "no items matched path '{path}' for metric '{metric}'"
        )),
    }
}

fn filter_entries<'a>(
    entries: &'a [BaselineEntry],
    path: Option<&str>,
    metric: Option<&str>,
) -> Vec<&'a BaselineEntry> {
    entries
        .iter()
        .filter(|e| path.is_none_or(|p| e.path.starts_with(p)))
        .filter(|e| metric.is_none_or(|m| e.metric == m))
        .collect()
}

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
        let result = match do_measure(root, &entry.path, &entry.metric, entry.aggregate, factory) {
            Ok(r) => r,
            Err(e) => {
                eprintln!(
                    "warning: could not measure {} ({}): {e}",
                    entry.path, entry.metric
                );
                continue;
            }
        };

        let current = result.value;
        let baseline = entry.value;
        let delta = current - baseline;

        let metrics = factory(root);
        let higher_is_worse = metrics
            .iter()
            .find(|m| m.name() == entry.metric)
            .map(|m| m.higher_is_worse())
            .unwrap_or(true);

        let status = if (delta).abs() < 1e-10 {
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
            aggregate: entry.aggregate.to_string(),
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

/// Check against a historical git ref.
fn check_against_ref(
    root: &Path,
    base_ref: &str,
    path_filter: Option<&str>,
    metric_filter: Option<&str>,
    factory: &MetricFactory,
) -> Result<CheckReport, String> {
    use std::process::Command;

    // Resolve the ref to a hash
    let hash_output = Command::new("git")
        .args(["rev-parse", "--verify", base_ref])
        .current_dir(root)
        .output()
        .map_err(|e| format!("failed to run git: {e}"))?;
    if !hash_output.status.success() {
        return Err(format!(
            "git ref '{base_ref}' not found: {}",
            String::from_utf8_lossy(&hash_output.stderr).trim()
        ));
    }
    let hash = String::from_utf8_lossy(&hash_output.stdout)
        .trim()
        .to_string();
    let short = &hash[..7.min(hash.len())];
    let worktree_name = format!("normalize-ratchet-wt-{short}");
    let worktree_path = std::env::temp_dir().join(&worktree_name);
    let worktree_str = worktree_path.to_string_lossy().to_string();

    // Clean up any stale worktree
    if worktree_path.exists() {
        let _ = Command::new("git")
            .args(["worktree", "remove", &worktree_str, "--force"])
            .current_dir(root)
            .output();
    }

    // Create worktree
    let add_output = Command::new("git")
        .args(["worktree", "add", "--detach", &worktree_str, &hash])
        .current_dir(root)
        .output()
        .map_err(|e| format!("failed to create worktree: {e}"))?;
    if !add_output.status.success() {
        return Err(format!(
            "git worktree add failed: {}",
            String::from_utf8_lossy(&add_output.stderr).trim()
        ));
    }

    // Measure baseline at ref
    let baseline_measurements = {
        let metrics = factory(&worktree_path);
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
            let m = metrics.iter().find(|x| x.name() == metric_name).unwrap();
            if let Ok(all) = m.measure_all(&worktree_path) {
                for (addr, val) in all {
                    if path_filter.is_none_or(|p| addr.starts_with(p)) {
                        measurements.push((addr, metric_name.to_string(), val));
                    }
                }
            }
        }
        measurements
    };

    // Clean up worktree
    let _ = Command::new("git")
        .args(["worktree", "remove", &worktree_str, "--force"])
        .current_dir(root)
        .output();

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
            Err(_) => continue,
        };
        let current_map: std::collections::HashMap<&str, f64> =
            current_all.iter().map(|(k, v)| (k.as_str(), *v)).collect();

        for (addr, baseline_val) in baseline_items {
            let current_val = match current_map.get(addr.as_str()) {
                Some(&v) => v,
                None => continue,
            };
            let delta = current_val - baseline_val;
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
                aggregate: "raw".to_string(),
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
fn measure_at_ref(
    root: &Path,
    base_ref: &str,
    path: &str,
    metric: &str,
    agg: Aggregate,
    factory: &MetricFactory,
) -> Result<MeasureResult, String> {
    use std::process::Command;

    let hash_output = Command::new("git")
        .args(["rev-parse", "--verify", base_ref])
        .current_dir(root)
        .output()
        .map_err(|e| format!("failed to run git: {e}"))?;
    if !hash_output.status.success() {
        return Err(format!("git ref '{base_ref}' not found"));
    }
    let hash = String::from_utf8_lossy(&hash_output.stdout)
        .trim()
        .to_string();
    let short = &hash[..7.min(hash.len())];
    let worktree_name = format!("normalize-ratchet-wt-{short}");
    let worktree_path = std::env::temp_dir().join(&worktree_name);
    let worktree_str = worktree_path.to_string_lossy().to_string();

    if worktree_path.exists() {
        let _ = Command::new("git")
            .args(["worktree", "remove", &worktree_str, "--force"])
            .current_dir(root)
            .output();
    }

    let add_output = Command::new("git")
        .args(["worktree", "add", "--detach", &worktree_str, &hash])
        .current_dir(root)
        .output()
        .map_err(|e| format!("failed to create worktree: {e}"))?;
    if !add_output.status.success() {
        return Err(format!(
            "git worktree add failed: {}",
            String::from_utf8_lossy(&add_output.stderr).trim()
        ));
    }

    let result = do_measure(&worktree_path, path, metric, agg, factory);

    let _ = Command::new("git")
        .args(["worktree", "remove", &worktree_str, "--force"])
        .current_dir(root)
        .output();

    result
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
pub fn build_ratchet_diagnostics(
    root: &Path,
    factory: &MetricFactory,
) -> normalize_output::diagnostics::DiagnosticsReport {
    use normalize_output::diagnostics::{DiagnosticsReport, Issue, Severity};

    let baseline = match load_baseline(root) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("ratchet: could not load baseline: {e}");
            return DiagnosticsReport::new();
        }
    };

    if baseline.entries.is_empty() {
        return DiagnosticsReport::new();
    }

    let entries: Vec<&BaselineEntry> = baseline.entries.iter().collect();
    let report = match build_check_report(root, entries, factory) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("ratchet: check failed: {e}");
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
    }
}
