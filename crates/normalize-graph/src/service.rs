//! CLI service for the `graph` verb.
//!
//! Implements `normalize graph` (module/symbol/type graph analysis),
//! `graph dependents`, and `graph import-path` via the server-less `#[cli]`
//! pattern.
//!
//! The service owns its config access: it loads the `[index]`, `[walk]`, and
//! `[pretty]` sections directly from the global and project `config.toml` files
//! (mirroring the "sessions technique"), so this crate does not depend on the
//! main crate's monolithic `NormalizeConfig`. Index acquisition goes through
//! `normalize_index::require_import_graph`, which takes the config **slices**.

use crate::report::{DependentsReport, GraphReport, GraphTarget, ImportPathReport};
use normalize_index::{IndexConfig, require_import_graph};
use normalize_output::{OutputFormatter, PrettyConfig};
use normalize_rules_config::WalkConfig;
use server_less::cli;
use std::cell::Cell;
use std::path::Path;

/// The `[index]` + `[walk]` + `[pretty]` config sections this service reads.
#[derive(Default)]
struct GraphConfig {
    index: IndexConfig,
    walk: WalkConfig,
    pretty: PrettyConfig,
}

/// Load the relevant config sections from the global then project `config.toml`.
///
/// Delegates to the shared [`normalize_config_paths::ConfigSlices`] loader, which
/// applies per-section last-wins precedence (project overrides global; a project
/// config that omits a section keeps the global one) — matching the main crate's
/// `NormalizeConfig::load` exactly.
fn load_config(root: &Path) -> GraphConfig {
    let slices = normalize_config_paths::ConfigSlices::load(root);
    GraphConfig {
        index: slices.slice("index"),
        walk: slices.slice("walk"),
        pretty: slices.slice("pretty"),
    }
}

/// CLI service implementing `normalize graph` subcommands.
pub struct GraphService {
    pretty: Cell<bool>,
    pretty_raw: Cell<bool>,
    compact_raw: Cell<bool>,
}

impl GraphService {
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
    fn resolve_format(&self, cfg: &GraphConfig) {
        let is_pretty = !self.compact_raw.get() && (self.pretty_raw.get() || cfg.pretty.enabled());
        self.pretty.set(is_pretty);
    }

    /// Acquire the index, ensuring it holds import-graph data.
    async fn acquire(
        &self,
        root: &Path,
        cfg: &GraphConfig,
    ) -> Result<normalize_index::FileIndex, String> {
        require_import_graph(root, &cfg.index, &cfg.walk).await
    }

    fn display_output<T: OutputFormatter>(&self, r: &T) -> String {
        if self.pretty.get() {
            r.format_pretty()
        } else {
            r.format_text()
        }
    }

    fn display_graph(&self, r: &GraphReport) -> String {
        self.display_output(r)
    }

    fn display_dependents(&self, r: &DependentsReport) -> String {
        self.display_output(r)
    }

    fn display_import_path(&self, r: &ImportPathReport) -> String {
        self.display_output(r)
    }
}

impl server_less::CliGlobals for GraphService {
    fn set_global_flag(&self, name: &str, value: bool) {
        match name {
            "pretty" => self.pretty_raw.set(value),
            "compact" => self.compact_raw.set(value),
            _ => {}
        }
    }
}

#[cli(
    name = "graph",
    description = "Analyze the dependency graph: cycles, blast radius, import paths. Requires the facts index.",
    global = [
        pretty = "Human-friendly output with colors and formatting",
        compact = "Compact output without colors (overrides TTY detection)",
    ]
)]
impl GraphService {
    /// Graph-theoretic properties of the dependency graph (requires facts index)
    ///
    /// Reports dependency cycles (circular imports), hub modules (high fan-in/fan-out),
    /// and graph centrality. Also known as: circular dependency detection, import cycle finder.
    ///
    /// Examples:
    ///   normalize graph                     # module dependency graph
    ///   normalize graph --on symbols        # symbol-level graph
    #[cli(default, display_with = "display_graph")]
    pub async fn graph(
        &self,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
        #[param(short = 'l', help = "Max examples per section (0=no limit)")] limit: Option<usize>,
        #[param(help = "Graph nodes: modules (default) or symbols")] on: Option<GraphTarget>,
    ) -> Result<GraphReport, String> {
        let root_path = Self::root_path(root)?;
        let cfg = load_config(&root_path);
        self.resolve_format(&cfg);
        let effective_limit = match limit.unwrap_or(10) {
            0 => usize::MAX,
            n => n,
        };
        let target = on.unwrap_or(GraphTarget::Modules);
        let idx = self.acquire(&root_path, &cfg).await?;
        crate::report::analyze_graph(&idx, effective_limit, target)
            .await
            .map_err(|e| format!("Graph analysis failed: {}", e))
    }

    /// Reverse-dependency closure: who imports this file or module? (requires facts index)
    ///
    /// Also known as: blast radius, impact analysis, reverse imports, dependents, upstream callers
    /// at the module level. Use this before deleting or changing a module's public API.
    ///
    /// Examples:
    ///   normalize graph dependents src/lib.rs           # modules that import lib.rs
    ///   normalize graph dependents src/lib.rs --on symbols  # symbol-level dependents
    #[cli(display_with = "display_dependents")]
    pub async fn dependents(
        &self,
        #[param(positional, help = "File or module to find dependents for")] target: String,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
        #[param(help = "Graph nodes: modules (default), symbols, or types")] on: Option<
            GraphTarget,
        >,
    ) -> Result<DependentsReport, String> {
        let root_path = Self::root_path(root)?;
        let cfg = load_config(&root_path);
        self.resolve_format(&cfg);
        let graph_target = on.unwrap_or(GraphTarget::Modules);
        let idx = self.acquire(&root_path, &cfg).await?;
        crate::report::analyze_dependents(&idx, &target, graph_target)
            .await
            .map_err(|e| format!("Dependents query failed: {}", e))
    }

    /// Find the shortest import chain between two files (requires facts index)
    ///
    /// Uses BFS over the resolved import graph to find the path from `<from>` to `<to>`.
    /// Also known as: dependency path, import chain, module reachability.
    ///
    /// Examples:
    ///   normalize graph import-path src/a.rs src/b.rs           # shortest path
    ///   normalize graph import-path src/a.rs src/b.rs --all     # all simple paths (up to 5)
    ///   normalize graph import-path src/a.rs src/b.rs --reverse # path from b to a
    #[cli(display_with = "display_import_path")]
    pub async fn import_path(
        &self,
        #[param(positional, help = "Source file (root-relative or absolute path)")] from: String,
        #[param(positional, help = "Target file (root-relative or absolute path)")] to: String,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
        #[param(help = "Show all simple paths instead of just the shortest (up to --limit)")]
        all: bool,
        #[param(
            short = 'l',
            help = "Maximum number of paths to return with --all (default: 5)"
        )]
        limit: Option<usize>,
        #[param(help = "Find paths from <to> to <from> instead")] reverse: bool,
    ) -> Result<ImportPathReport, String> {
        let root_path = Self::root_path(root)?;
        let cfg = load_config(&root_path);
        self.resolve_format(&cfg);
        let path_limit = limit.unwrap_or(5);
        let idx = self.acquire(&root_path, &cfg).await?;
        crate::report::find_import_path_command(
            &idx, &root_path, &from, &to, all, path_limit, reverse,
        )
        .await
        .map_err(|e| format!("Import path query failed: {}", e))
    }
}
