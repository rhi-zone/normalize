//! CLI service for the `architecture` verb.
//!
//! Implements `normalize architecture` (coupling, cross-imports, hub modules),
//! `architecture layering`, and `architecture depth-map` via the server-less
//! `#[cli]` pattern.
//!
//! The service owns its config access: it loads the `[index]`, `[walk]`, and
//! `[pretty]` sections directly from the global and project `config.toml` files
//! (mirroring the "sessions technique"), so this crate does not depend on the
//! main crate's monolithic `NormalizeConfig`. Index acquisition goes through
//! `normalize_index::{require_import_graph, ensure_ready}`, which take the config
//! **slices**.

use crate::architecture::ArchitectureReport;
use crate::depth_map::DepthMapReport;
use crate::layering::LayeringReport;
use normalize_index::IndexConfig;
use normalize_output::{OutputFormatter, PrettyConfig};
use normalize_rules_config::WalkConfig;
use server_less::cli;
use std::cell::Cell;
use std::path::Path;

/// The `[index]` + `[walk]` + `[pretty]` config sections this service reads.
#[derive(serde::Deserialize, Default)]
struct ArchConfig {
    #[serde(default)]
    index: IndexConfig,
    #[serde(default)]
    walk: WalkConfig,
    #[serde(default)]
    pretty: PrettyConfig,
}

/// Load the relevant config sections from the global then project `config.toml`.
///
/// Later files override earlier ones (project overrides global), matching the
/// precedence the main crate's `NormalizeConfig::load` uses.
fn load_config(root: &Path) -> ArchConfig {
    let global = std::env::var("XDG_CONFIG_HOME")
        .map(std::path::PathBuf::from)
        .ok()
        .or_else(|| dirs::home_dir().map(|h| h.join(".config")))
        .map(|c| c.join("normalize").join("config.toml"));

    let mut cfg = ArchConfig::default();
    for path in [global, Some(root.join(".normalize").join("config.toml"))]
        .into_iter()
        .flatten()
    {
        if let Ok(content) = std::fs::read_to_string(&path)
            && let Ok(parsed) = toml::from_str::<ArchConfig>(&content)
        {
            cfg = parsed;
        }
    }
    cfg
}

/// CLI service implementing `normalize architecture` subcommands.
pub struct ArchitectureService {
    pretty: Cell<bool>,
    pretty_raw: Cell<bool>,
    compact_raw: Cell<bool>,
}

impl ArchitectureService {
    pub fn new(pretty: &Cell<bool>) -> Self {
        Self {
            pretty: Cell::new(pretty.get()),
            pretty_raw: Cell::new(false),
            compact_raw: Cell::new(false),
        }
    }

    fn root_path(root: Option<String>) -> Result<std::path::PathBuf, String> {
        root.map(std::path::PathBuf::from).map_or_else(
            || std::env::current_dir().map_err(|e| format!("failed to get working directory: {e}")),
            Ok,
        )
    }

    /// Resolve pretty mode from CLI flags plus the project's `[pretty]` config.
    fn resolve_format(&self, cfg: &ArchConfig) {
        let is_pretty = !self.compact_raw.get() && (self.pretty_raw.get() || cfg.pretty.enabled());
        self.pretty.set(is_pretty);
    }

    /// Acquire the index, ensuring it holds import-graph data.
    async fn acquire(
        &self,
        root: &Path,
        cfg: &ArchConfig,
    ) -> Result<normalize_index::FileIndex, String> {
        normalize_index::require_import_graph(root, &cfg.index, &cfg.walk).await
    }

    fn display_output<T: OutputFormatter>(&self, r: &T) -> String {
        if self.pretty.get() {
            r.format_pretty()
        } else {
            r.format_text()
        }
    }

    fn display_architecture(&self, r: &ArchitectureReport) -> String {
        self.display_output(r)
    }

    fn display_depth_map(&self, r: &DepthMapReport) -> String {
        self.display_output(r)
    }

    fn display_layering(&self, r: &LayeringReport) -> String {
        self.display_output(r)
    }
}

impl server_less::CliGlobals for ArchitectureService {
    fn set_global_flag(&self, name: &str, value: bool) {
        match name {
            "pretty" => self.pretty_raw.set(value),
            "compact" => self.compact_raw.set(value),
            _ => {}
        }
    }
}

#[cli(
    name = "architecture",
    description = "Analyze architectural structure: coupling, dependency cycles, hub modules, layering, and depth. Requires the facts index.",
    global = [
        pretty = "Human-friendly output with colors and formatting",
        compact = "Compact output without colors (overrides TTY detection)",
    ]
)]
impl ArchitectureService {
    /// Analyze architectural structure: coupling, dependency cycles, and hub modules.
    ///
    /// Detects circular imports (dependency cycles), highly-coupled module pairs, and hub
    /// modules (high fan-in or fan-out). Also known as: circular dependency detection,
    /// architecture health, import cycle finder, god module detection.
    ///
    /// Requires the facts index (`normalize structure rebuild`). Returns an `ArchitectureReport`
    /// with coupling pairs, cycle lists, and hub modules ranked by fan-in/fan-out.
    ///
    /// Examples:
    ///   normalize architecture              # coupling, hubs, layer flows
    ///   normalize architecture --limit 0    # no cap on cross-import entries
    #[cli(default, display_with = "display_architecture")]
    pub async fn architecture(
        &self,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
        #[param(
            short = 'l',
            help = "Maximum number of cross-import entries to include in output (0 = no limit, default 20)"
        )]
        limit: Option<usize>,
    ) -> Result<ArchitectureReport, String> {
        let root_path = Self::root_path(root)?;
        let cfg = load_config(&root_path);
        self.resolve_format(&cfg);
        let idx = self.acquire(&root_path, &cfg).await?;
        let mut report = crate::architecture::analyze_architecture(&idx)
            .await
            .map_err(|e| format!("Architecture analysis failed: {}", e))?;
        // Cap cross_imports to avoid bloated JSON output for agents.
        // Default cap is 20; --limit 0 disables the cap.
        let cap = match limit.unwrap_or(20) {
            0 => usize::MAX,
            n => n,
        };
        report.cross_imports.truncate(cap);
        Ok(report)
    }

    /// Rank modules by dependency depth and ripple risk in the import graph.
    ///
    /// Also known as: blast radius, change impact, ripple effect analysis, dependency depth.
    /// Modules deep in the import graph that are also widely imported have the highest ripple
    /// risk — changes to them affect many other modules.
    ///
    /// Metrics:
    /// - **Depth**: longest chain of transitive importers reaching this module (0 = entry point,
    ///   nothing imports it).
    /// - **Downstream**: transitive reverse-dependency count (BFS through importers).
    /// - **Ripple Score**: `fan_out × depth × downstream` — composite blast-radius estimate.
    ///
    /// Requires the facts index (`normalize structure rebuild`). Returns a `DepthMapReport`
    /// sorted by ripple score (highest first).
    #[cli(display_with = "display_depth_map")]
    pub async fn depth_map(
        &self,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
        #[param(short = 'l', help = "Maximum number of modules to show (0=no limit)")]
        limit: Option<usize>,
        #[param(help = "Show delta vs this git ref (branch, tag, commit, HEAD~N)")] diff: Option<
            String,
        >,
    ) -> Result<DepthMapReport, String> {
        let root_path = Self::root_path(root)?;
        let cfg = load_config(&root_path);
        self.resolve_format(&cfg);
        let effective_limit = match limit.unwrap_or(30) {
            0 => usize::MAX,
            n => n,
        };
        let idx = self.acquire(&root_path, &cfg).await?;
        let mut report = crate::depth_map::analyze_depth_map(&idx, effective_limit)
            .await
            .map_err(|e| format!("Depth map analysis failed: {}", e))?;
        if let Some(ref diff_ref) = diff {
            use normalize_git::{resolve_ref, run_in_worktree};
            use normalize_rank::ranked::compute_ranked_diff;
            let hash = resolve_ref(&root_path, diff_ref)?;
            let baseline = run_in_worktree(&root_path, &hash, |wt| {
                let handle = tokio::runtime::Handle::current();
                tokio::task::block_in_place(|| {
                    handle.block_on(async {
                        let wt_cfg = load_config(wt);
                        let wt_idx =
                            normalize_index::ensure_ready(wt, &wt_cfg.index, &wt_cfg.walk).await?;
                        crate::depth_map::analyze_depth_map(&wt_idx, usize::MAX)
                            .await
                            .map_err(|e| format!("Baseline depth map failed: {}", e))
                    })
                })
            })?;
            compute_ranked_diff(&mut report.modules, &baseline.modules);
            report.diff_ref = Some(diff_ref.clone());
        }
        Ok(report)
    }

    /// Rank modules by import layering compliance: do imports flow in one direction?
    ///
    /// Also known as: dependency direction violations, upward imports, architecture violations,
    /// circular dependency detection at the layer level. Detects imports that violate
    /// a clean layered architecture (e.g., core importing from UI).
    ///
    /// Layer is inferred from the first directory component of each module path. Imports are
    /// classified as:
    /// - **Downward**: importing a module in a deeper layer (good — correct direction).
    /// - **Upward**: importing a module in a shallower layer (bad — coupling violation).
    /// - **Same Layer**: importing a module in the same layer (neutral).
    ///
    /// Compliance = `downward / (downward + upward)` for cross-layer imports; 1.0 if there are
    /// no cross-layer imports. Modules are ranked worst-first (lowest compliance).
    ///
    /// Requires the facts index (`normalize structure rebuild`). Returns a `LayeringReport`
    /// with per-module violation counts and a per-layer summary.
    #[cli(display_with = "display_layering")]
    pub async fn layering(
        &self,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
        #[param(short = 'l', help = "Maximum number of modules to show (0=no limit)")]
        limit: Option<usize>,
        #[param(help = "Show delta vs this git ref (branch, tag, commit, HEAD~N)")] diff: Option<
            String,
        >,
    ) -> Result<LayeringReport, String> {
        let root_path = Self::root_path(root)?;
        let cfg = load_config(&root_path);
        self.resolve_format(&cfg);
        let effective_limit = match limit.unwrap_or(30) {
            0 => usize::MAX,
            n => n,
        };
        let idx = self.acquire(&root_path, &cfg).await?;
        let mut report = crate::layering::analyze_layering(&idx, effective_limit)
            .await
            .map_err(|e| format!("Layering analysis failed: {}", e))?;
        if let Some(ref diff_ref) = diff {
            use normalize_git::{resolve_ref, run_in_worktree};
            use normalize_rank::ranked::compute_ranked_diff;
            let hash = resolve_ref(&root_path, diff_ref)?;
            let baseline = run_in_worktree(&root_path, &hash, |wt| {
                let handle = tokio::runtime::Handle::current();
                tokio::task::block_in_place(|| {
                    handle.block_on(async {
                        let wt_cfg = load_config(wt);
                        let wt_idx =
                            normalize_index::ensure_ready(wt, &wt_cfg.index, &wt_cfg.walk).await?;
                        crate::layering::analyze_layering(&wt_idx, usize::MAX)
                            .await
                            .map_err(|e| format!("Baseline layering failed: {}", e))
                    })
                })
            })?;
            compute_ranked_diff(&mut report.modules, &baseline.modules);
            report.diff_ref = Some(diff_ref.clone());
        }
        Ok(report)
    }
}
