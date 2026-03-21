//! CLI service for ratchet commands.
//!
//! Provides `normalize ratchet check`, `normalize ratchet update`, and
//! `normalize ratchet show` via the server-less `#[cli]` macro.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use normalize_output::OutputFormatter;
use server_less::cli;

use crate::Metric;
use crate::baseline::{self, Baseline};
use crate::check::{Regression, check_against_baseline};
use crate::update::{UpdateSummary, compute_update};

/// Report returned by `normalize ratchet check`.
#[derive(Debug, serde::Serialize, schemars::JsonSchema)]
pub struct RatchetCheckReport {
    /// All regressions detected.
    pub regressions: Vec<Regression>,
    /// New metric keys not yet in the baseline.
    pub new_keys: Vec<(String, String)>,
    /// Baseline keys that are no longer measured.
    pub removed_keys: Vec<(String, String)>,
    /// Total keys compared.
    pub keys_checked: usize,
    /// Git ref used as the baseline (if `--base` was supplied).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_ref: Option<String>,
}

impl OutputFormatter for RatchetCheckReport {
    fn format_text(&self) -> String {
        let mut out = String::new();

        if self.regressions.is_empty() {
            out.push_str("No regressions detected");
            if let Some(ref r) = self.base_ref {
                out.push_str(&format!(" (vs {r})"));
            }
            out.push('\n');
        } else {
            out.push_str(&format!(
                "{} regression{} detected",
                self.regressions.len(),
                if self.regressions.len() == 1 { "" } else { "s" }
            ));
            if let Some(ref r) = self.base_ref {
                out.push_str(&format!(" (vs {r})"));
            }
            out.push('\n');
            for reg in &self.regressions {
                out.push_str(&format!(
                    "  {} / {} : {} -> {} (delta +{})\n",
                    reg.metric, reg.key, reg.baseline, reg.current, reg.delta
                ));
            }
        }

        if !self.new_keys.is_empty() {
            out.push_str(&format!(
                "\n{} new key{} (not yet in baseline):\n",
                self.new_keys.len(),
                if self.new_keys.len() == 1 { "" } else { "s" }
            ));
            for (m, k) in &self.new_keys {
                out.push_str(&format!("  {m} / {k}\n"));
            }
        }

        if !self.removed_keys.is_empty() {
            out.push_str(&format!(
                "\n{} removed key{} (in baseline, no longer measured):\n",
                self.removed_keys.len(),
                if self.removed_keys.len() == 1 {
                    ""
                } else {
                    "s"
                }
            ));
            for (m, k) in &self.removed_keys {
                out.push_str(&format!("  {m} / {k}\n"));
            }
        }

        out.push_str(&format!(
            "\n{} key{} checked\n",
            self.keys_checked,
            if self.keys_checked == 1 { "" } else { "s" }
        ));
        out
    }
}

/// Report returned by `normalize ratchet update`.
#[derive(Debug, serde::Serialize, schemars::JsonSchema)]
pub struct RatchetUpdateReport {
    /// Path to the updated baseline file.
    pub path: String,
    /// Whether `--force` was used.
    pub force: bool,
    /// Whether this was a dry run (file not written).
    pub dry_run: bool,
    /// Summary of changes made.
    pub summary: UpdateSummary,
}

impl OutputFormatter for RatchetUpdateReport {
    fn format_text(&self) -> String {
        let mut out = String::new();
        if self.dry_run {
            out.push_str("[dry-run] ");
        }
        out.push_str(&format!("Updated {}\n", self.path));
        let s = &self.summary;
        if s.added > 0 {
            out.push_str(&format!("  added:     {}\n", s.added));
        }
        if s.lowered > 0 {
            out.push_str(&format!("  lowered:   {}\n", s.lowered));
        }
        if s.raised > 0 {
            out.push_str(&format!("  raised:    {} (--force)\n", s.raised));
        }
        if s.removed > 0 {
            out.push_str(&format!("  removed:   {}\n", s.removed));
        }
        if s.unchanged > 0 {
            out.push_str(&format!("  unchanged: {}\n", s.unchanged));
        }
        out
    }
}

/// Report returned by `normalize ratchet show`.
#[derive(Debug, serde::Serialize, schemars::JsonSchema)]
pub struct RatchetShowReport {
    /// Path to the baseline file.
    pub path: String,
    /// Whether the file exists.
    pub exists: bool,
    /// The baseline data (None if file does not exist).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub baseline: Option<Baseline>,
}

impl OutputFormatter for RatchetShowReport {
    fn format_text(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!("{}\n", self.path));
        let Some(ref b) = self.baseline else {
            out.push_str("  (no baseline — run `normalize ratchet update` to create one)\n");
            return out;
        };
        for (metric_name, entries) in &b.metrics {
            out.push_str(&format!("\n[{metric_name}]\n"));
            // Sort for stable output: ::total last, rest alphabetically
            let mut pairs: Vec<(&String, &i64)> = entries.iter().collect();
            pairs.sort_by(|(a, _), (b, _)| {
                let a_total = a.as_str() == crate::complexity::TOTAL_KEY;
                let b_total = b.as_str() == crate::complexity::TOTAL_KEY;
                match (a_total, b_total) {
                    (true, false) => std::cmp::Ordering::Greater,
                    (false, true) => std::cmp::Ordering::Less,
                    _ => a.cmp(b),
                }
            });
            for (k, v) in pairs {
                out.push_str(&format!("  {k}: {v}\n"));
            }
        }
        out
    }
}

// ---------------------------------------------------------------------------
// Service
// ---------------------------------------------------------------------------

pub use crate::MetricFactory;

/// Internal result type for measure_all to avoid overly complex type signature.
type MeasureAllResult = (Vec<Box<dyn Metric>>, HashMap<String, Vec<(String, i64)>>);

/// Ratchet sub-service: check, update, and show metric baselines.
pub struct RatchetService {
    metric_factory: MetricFactory,
}

impl RatchetService {
    /// Create a new `RatchetService` backed by the given metric factory.
    pub fn new(metric_factory: MetricFactory) -> Self {
        Self { metric_factory }
    }

    fn root_path(root: Option<String>) -> PathBuf {
        root.map(PathBuf::from)
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
    }

    /// Measure all metrics from the factory against `root`.
    fn measure_all(&self, root: &Path) -> Result<MeasureAllResult, String> {
        let metrics = (self.metric_factory)();
        let mut measurements = HashMap::new();
        for metric in &metrics {
            let entries = metric
                .measure(root)
                .map_err(|e| format!("failed to measure {}: {e}", metric.name()))?;
            measurements.insert(metric.name().to_string(), entries);
        }
        Ok((metrics, measurements))
    }

    fn display_check(&self, r: &RatchetCheckReport) -> String {
        r.format_text()
    }

    fn display_update(&self, r: &RatchetUpdateReport) -> String {
        r.format_text()
    }

    fn display_show(&self, r: &RatchetShowReport) -> String {
        r.format_text()
    }
}

#[cli]
impl RatchetService {
    /// Compare current metrics to the stored baseline, reporting regressions.
    ///
    /// Without `--base`, reads `.normalize/ratchet.json` as the baseline.
    /// With `--base <ref>`, measures at that git ref as the baseline and
    /// compares to the current working tree.
    ///
    /// Exits with a non-zero status if regressions are found.
    #[cli(display_with = "display_check")]
    pub fn check(
        &self,
        #[param(positional, help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
        #[param(
            help = "Use this git ref as the baseline instead of .normalize/ratchet.json"
        )]
        base: Option<String>,
    ) -> Result<RatchetCheckReport, String> {
        let root_path = Self::root_path(root);

        let (metrics, current_measurements) = self.measure_all(&root_path)?;
        let metric_refs: Vec<&dyn Metric> = metrics.iter().map(|m| m.as_ref()).collect();

        let baseline = if let Some(ref git_ref) = base {
            // Measure at the given git ref using a temporary worktree
            measure_at_ref(&root_path, git_ref, &metric_refs)?
        } else {
            baseline::load(&root_path).map_err(|e| e.to_string())?
        };

        let result = check_against_baseline(&baseline, &current_measurements, &metric_refs);

        let report = RatchetCheckReport {
            regressions: result.regressions,
            new_keys: result.new_keys,
            removed_keys: result.removed_keys,
            keys_checked: result.keys_checked,
            base_ref: base,
        };

        if !report.regressions.is_empty() {
            let detail = report.format_text();
            return Err(format!(
                "{detail}{} regression(s) found",
                report.regressions.len()
            ));
        }

        Ok(report)
    }

    /// Re-measure metrics and update `.normalize/ratchet.json`.
    ///
    /// Default (no `--force`): only lowers values or adds new entries (true ratchet).
    /// With `--force`: allows raising values too and removes stale keys.
    #[cli(display_with = "display_update")]
    pub fn update(
        &self,
        #[param(positional, help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
        #[param(help = "Allow raising values (not just lowering)")] force: bool,
        #[param(help = "Show what would change without writing the file")] dry_run: bool,
    ) -> Result<RatchetUpdateReport, String> {
        let root_path = Self::root_path(root);

        let (metrics, current_measurements) = self.measure_all(&root_path)?;
        let metric_refs: Vec<&dyn Metric> = metrics.iter().map(|m| m.as_ref()).collect();

        let existing = baseline::load(&root_path).map_err(|e| e.to_string())?;
        let update = compute_update(&existing, &current_measurements, &metric_refs, force);

        let path = baseline::baseline_path(&root_path)
            .to_string_lossy()
            .to_string();

        if !dry_run {
            baseline::save(&root_path, &update.baseline).map_err(|e| e.to_string())?;
        }

        Ok(RatchetUpdateReport {
            path,
            force,
            dry_run,
            summary: update.summary,
        })
    }

    /// Display the current baseline stored in `.normalize/ratchet.json`.
    #[cli(display_with = "display_show")]
    pub fn show(
        &self,
        #[param(positional, help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
    ) -> Result<RatchetShowReport, String> {
        let root_path = Self::root_path(root);
        let path = baseline::baseline_path(&root_path)
            .to_string_lossy()
            .to_string();
        let exists = baseline::baseline_path(&root_path).exists();
        let baseline = if exists {
            Some(baseline::load(&root_path).map_err(|e| e.to_string())?)
        } else {
            None
        };
        Ok(RatchetShowReport {
            path,
            exists,
            baseline,
        })
    }
}

/// Measure all metrics at a git ref by creating a temporary worktree.
fn measure_at_ref(root: &Path, git_ref: &str, metrics: &[&dyn Metric]) -> Result<Baseline, String> {
    use std::process::Command;

    // Resolve ref to a full commit hash
    let output = Command::new("git")
        .args(["rev-parse", "--verify", git_ref])
        .current_dir(root)
        .output()
        .map_err(|e| format!("failed to run git rev-parse: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "git ref '{}' not found: {}",
            git_ref,
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    let hash = String::from_utf8_lossy(&output.stdout).trim().to_string();
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

    // Measure at the worktree
    let result: Result<Baseline, String> = (|| {
        let mut b = Baseline::default();
        for metric in metrics {
            let entries = metric
                .measure(&worktree_path)
                .map_err(|e| format!("failed to measure {} at {}: {e}", metric.name(), git_ref))?;
            b.metrics
                .insert(metric.name().to_string(), entries.into_iter().collect());
        }
        Ok(b)
    })();

    // Always remove the worktree
    let _ = Command::new("git")
        .args(["worktree", "remove", &worktree_str, "--force"])
        .current_dir(root)
        .output();

    result
}
