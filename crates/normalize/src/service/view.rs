//! View sub-service for server-less CLI.
//!
//! Hosts the default `view` command (directory/file/symbol navigation) and graph
//! navigation subcommands: referenced-by, references, history, dependents, trace, graph, blame.

use crate::commands::analyze::call_graph::CallEntry;
use crate::commands::analyze::graph::{DependentsReport, GraphReport, GraphTarget};
use crate::commands::analyze::provenance::ProvenanceReport;
use crate::commands::view::report::{ViewHistoryReport, ViewListReport, ViewReport};
use crate::output::OutputFormatter;
use server_less::cli;
use std::cell::Cell;
use std::path::PathBuf;

/// Report for `normalize view trace` — value provenance trace for a symbol.
#[derive(serde::Serialize, schemars::JsonSchema)]
pub struct TraceReport {
    /// Human-readable trace output showing value provenance for the requested symbol.
    pub trace: String,
}

impl OutputFormatter for TraceReport {
    fn format_text(&self) -> String {
        self.trace.clone()
    }
}

impl std::fmt::Display for TraceReport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.trace)
    }
}

/// View sub-service: directory/file/symbol navigation and graph navigation.
pub struct ViewService {
    pretty: Cell<bool>,
    /// Text prefix to prepend to the default view output (used for --dir-context).
    view_prefix: Cell<String>,
}

impl ViewService {
    pub fn new(pretty: &Cell<bool>) -> Self {
        Self {
            pretty: Cell::new(pretty.get()),
            view_prefix: Cell::new(String::new()),
        }
    }

    fn root_path(root: Option<String>) -> Result<PathBuf, String> {
        root.map(PathBuf::from).map_or_else(
            || std::env::current_dir().map_err(|e| format!("failed to get working directory: {e}")),
            Ok,
        )
    }

    fn resolve_format(&self, pretty: bool, compact: bool, root: &std::path::Path) {
        use crate::config::NormalizeConfig;
        let config = NormalizeConfig::load(root);
        let is_pretty = !compact && (pretty || config.pretty.enabled());
        self.pretty.set(is_pretty);
    }

    fn display_output<T: OutputFormatter>(&self, r: &T) -> String {
        if self.pretty.get() {
            r.format_pretty()
        } else {
            r.format_text()
        }
    }

    fn display_view(&self, value: &ViewReport) -> String {
        let prefix = self.view_prefix.take();
        let text = self.display_output(value);
        if prefix.is_empty() {
            text
        } else {
            format!("{}{}", prefix, text)
        }
    }

    #[allow(dead_code)] // called by server-less proc macro via display_with = "display_view_list"
    fn display_view_list(&self, value: &ViewListReport) -> String {
        self.display_output(value)
    }

    fn display_call_graph(&self, entries: &[CallEntry]) -> String {
        entries
            .iter()
            .map(|e| {
                let access_suffix = e
                    .access
                    .as_deref()
                    .map(|a| format!(" [{a}]"))
                    .unwrap_or_default();
                format!("  {}:{}:{}{}", e.file, e.line, e.symbol, access_suffix)
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn display_history(&self, r: &ViewHistoryReport) -> String {
        self.display_output(r)
    }

    fn display_dependents(&self, r: &DependentsReport) -> String {
        self.display_output(r)
    }

    fn display_trace(&self, r: &TraceReport) -> String {
        r.trace.clone()
    }

    fn display_graph(&self, r: &GraphReport) -> String {
        self.display_output(r)
    }

    fn display_provenance(&self, r: &ProvenanceReport) -> String {
        self.display_output(r)
    }
}

#[cli(
    name = "view",
    description = "View a node in the codebase tree, or navigate symbol relationships",
    global = [
        pretty = "Human-friendly output with colors and formatting",
        compact = "Compact output without colors (overrides TTY detection)",
    ]
)]
impl ViewService {
    /// View a node in the codebase tree (directory, file, or symbol)
    ///
    /// Examples:
    ///   normalize view                           # top-level directory tree
    ///   normalize view src/                      # expand a subdirectory
    ///   normalize view src/main.rs               # file skeleton (functions, classes)
    ///   normalize view src/main.rs/ClassName     # single symbol and its children
    ///   normalize view SymbolName                # search by symbol name
    ///   normalize view file.rs:42                # jump to line 42
    ///   normalize view src/ --depth 2            # deeper expansion
    ///   normalize view src/main.rs --full        # full source code
    ///   normalize view src/main.rs --deps        # show imports/exports
    ///   normalize view src/main.rs --context     # skeleton + imports combined
    ///   normalize view referenced-by MyFn        # show callers of MyFn
    ///   normalize view references MyFn           # show what MyFn calls
    ///   normalize view history src/main.rs/MyFn  # git history for a symbol
    ///   normalize view dependents src/lib.rs     # show what depends on this file
    #[cli(default, display_with = "display_view")]
    #[allow(clippy::too_many_arguments)]
    pub async fn view(
        &self,
        #[param(
            positional,
            help = "Target: path, path/Symbol, Parent/method, file:line, or SymbolName"
        )]
        target: Option<String>,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
        #[param(
            short = 'd',
            help = "Depth of expansion (0=names only, 1=signatures, 2=with children, -1=all)"
        )]
        depth: Option<i32>,
        #[param(short = 'n', help = "Show line numbers")] line_numbers: bool,
        #[param(help = "Show dependencies (imports/exports)")] deps: bool,
        #[param(short = 'k', help = "Filter by symbol kind: class, function, method")] kind: Option<
            crate::commands::view::tree::SymbolKindFilter,
        >,
        #[param(help = "Show only type definitions")] types_only: bool,
        #[param(help = "Include test functions and test modules")] tests: bool,
        #[param(help = "Disable smart display (no collapsing single-child dirs)")] raw: bool,
        #[param(help = "Focus view on module")] focus: Option<String>,
        #[param(help = "Inline signatures of specific imported symbols")] resolve_imports: bool,
        #[param(help = "Show full source code")] full: bool,
        #[param(help = "Show full docstrings")] docs: bool,
        #[param(help = "Hide all docstrings")] no_docs: bool,
        #[param(help = "Hide parent/ancestor context")] no_parent: bool,
        #[param(help = "Context view: skeleton + imports combined")] context: bool,
        #[param(
            help = "Prepend ancestor directory context files; value = max ancestor levels (e.g. --dir-context 2)"
        )]
        dir_context: Option<u32>,
        #[param(help = "Exclude paths matching pattern")] exclude: Vec<String>,
        #[param(help = "Include only paths matching pattern")] only: Vec<String>,
        #[param(short = 'i', help = "Case-insensitive symbol matching")] case_insensitive: bool,
        pretty: bool,
        compact: bool,
    ) -> Result<ViewReport, String> {
        let root_path = root
            .map(PathBuf::from)
            .map(Ok)
            .unwrap_or_else(std::env::current_dir)
            .map_err(|e| format!("Failed to get current directory: {e}"))?;

        self.resolve_format(pretty, compact, &root_path);

        let config = crate::config::NormalizeConfig::load(&root_path);

        let docstring_mode = if no_docs {
            crate::tree::DocstringDisplay::None
        } else if docs || config.view.show_docs() {
            crate::tree::DocstringDisplay::Full
        } else {
            crate::tree::DocstringDisplay::Summary
        };

        let ctx_files = config.view.context_files();

        // --dir-context N: prepend context files, walking up N ancestor levels (default: all)
        if let Some(depth) = dir_context {
            let target_path = target
                .as_ref()
                .map(|t| root_path.join(t))
                .unwrap_or_else(|| root_path.clone());
            let max_depth = if depth == 0 {
                None
            } else {
                Some(depth as usize)
            };
            if let Some(ctx) = crate::commands::context::get_merged_context(
                &root_path,
                &target_path,
                &ctx_files,
                max_depth,
            ) {
                self.view_prefix.set(format!("{}\n\n---\n\n", ctx));
            }
        }

        crate::commands::view::build_view_service(
            target.as_deref(),
            &root_path,
            depth.unwrap_or_else(|| config.view.depth()),
            line_numbers,
            deps,
            kind.as_ref(),
            types_only,
            tests,
            raw,
            focus.as_deref(),
            resolve_imports,
            full,
            docstring_mode,
            context,
            !no_parent,
            &exclude,
            &only,
            case_insensitive,
            &ctx_files,
        )
        .await
    }

    /// Show what references this symbol (callers in the call graph; requires facts index)
    ///
    /// Examples:
    ///   normalize view referenced-by MyFunction      # callers of a symbol
    ///   normalize view referenced-by file.rs#MyFn    # callers of a specific method
    #[cli(display_with = "display_call_graph")]
    pub async fn referenced_by(
        &self,
        #[param(positional, help = "Symbol to look up (or file#symbol)")] target: String,
        #[param(short = 'i', help = "Case-insensitive matching")] case_insensitive: bool,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
    ) -> Result<Vec<CallEntry>, String> {
        let root_path = Self::root_path(root)?;
        crate::commands::analyze::call_graph::build_call_graph(
            &root_path,
            &target,
            true,
            false,
            case_insensitive,
        )
        .await
    }

    /// List code entities matching filters in a scope
    ///
    /// Examples:
    ///   normalize view list                            # list top-level entries
    ///   normalize view list src/                       # list entries in src/
    ///   normalize view list --kind function            # list all functions
    ///   normalize view list src/ --kind class          # list classes in src/
    #[cli(display_with = "display_view_list")]
    #[allow(clippy::too_many_arguments)]
    pub async fn list(
        &self,
        #[param(positional, help = "Target scope (path, or symbol pattern)")] target: Option<
            String,
        >,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
        #[param(
            short = 'd',
            help = "Depth of expansion (0=names only, 1=signatures, 2=with children, -1=all)"
        )]
        depth: Option<i32>,
        #[param(short = 'k', help = "Filter by symbol kind: class, function, method")] kind: Option<
            crate::commands::view::tree::SymbolKindFilter,
        >,
        #[param(help = "Show only type definitions")] types_only: bool,
        #[param(help = "Include test functions and test modules")] tests: bool,
        #[param(help = "Disable smart display (no collapsing single-child dirs)")] raw: bool,
        #[param(help = "Show full docstrings")] docs: bool,
        #[param(help = "Hide all docstrings")] no_docs: bool,
        #[param(help = "Hide parent/ancestor context")] no_parent: bool,
        #[param(help = "Context view: skeleton + imports combined")] context: bool,
        #[param(help = "Exclude paths matching pattern")] exclude: Vec<String>,
        #[param(help = "Include only paths matching pattern")] only: Vec<String>,
        #[param(short = 'i', help = "Case-insensitive symbol matching")] case_insensitive: bool,
        pretty: bool,
        compact: bool,
    ) -> Result<ViewListReport, String> {
        let root_path = root
            .map(std::path::PathBuf::from)
            .map(Ok)
            .unwrap_or_else(std::env::current_dir)
            .map_err(|e| format!("Failed to get current directory: {e}"))?;

        self.resolve_format(pretty, compact, &root_path);

        let config = crate::config::NormalizeConfig::load(&root_path);

        let docstring_mode = if no_docs {
            crate::tree::DocstringDisplay::None
        } else if docs || config.view.show_docs() {
            crate::tree::DocstringDisplay::Full
        } else {
            crate::tree::DocstringDisplay::Summary
        };

        let ctx_files = config.view.context_files();

        crate::commands::view::build_view_list_service(
            target.as_deref(),
            &root_path,
            depth.unwrap_or_else(|| config.view.depth()),
            kind.as_ref(),
            types_only,
            tests,
            raw,
            false, // show_deps
            docstring_mode,
            context,
            !no_parent,
            &exclude,
            &only,
            case_insensitive,
            &ctx_files,
        )
        .await
    }

    /// Show what this symbol references (callees in the call graph; requires facts index)
    ///
    /// Examples:
    ///   normalize view references MyFunction     # functions called by MyFunction
    ///   normalize view references file.rs#MyFn  # references from a specific method
    #[cli(display_with = "display_call_graph")]
    pub async fn references(
        &self,
        #[param(positional, help = "Symbol to look up (or file#symbol)")] target: String,
        #[param(short = 'i', help = "Case-insensitive matching")] case_insensitive: bool,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
    ) -> Result<Vec<CallEntry>, String> {
        let root_path = Self::root_path(root)?;
        crate::commands::analyze::call_graph::build_call_graph(
            &root_path,
            &target,
            false,
            true,
            case_insensitive,
        )
        .await
    }

    /// Show git history for a symbol or file
    ///
    /// Examples:
    ///   normalize view history src/main.rs        # history for a file
    ///   normalize view history src/main.rs/MyFn   # history for a symbol
    ///   normalize view history --limit 20 src/main.rs  # last 20 commits
    #[cli(display_with = "display_history")]
    pub fn history(
        &self,
        #[param(positional, help = "Target: path, or path/Symbol")] target: String,
        #[param(short = 'n', help = "Maximum number of commits to show (default: 10)")]
        limit: Option<usize>,
        #[param(short = 'i', help = "Case-insensitive symbol matching")] case_insensitive: bool,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
    ) -> Result<ViewHistoryReport, String> {
        let root_path = Self::root_path(root)?;
        crate::commands::view::history::build_view_history_report(
            &target,
            &root_path,
            limit.unwrap_or(10),
            case_insensitive,
        )
    }

    /// Reverse-dependency closure: who imports this file or module? (requires facts index)
    ///
    /// Examples:
    ///   normalize view dependents src/lib.rs           # modules that import lib.rs
    ///   normalize view dependents src/lib.rs --on symbols  # symbol-level dependents
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
        pretty: bool,
        compact: bool,
    ) -> Result<DependentsReport, String> {
        let root_path = Self::root_path(root)?;
        self.resolve_format(pretty, compact, &root_path);
        let graph_target = on.unwrap_or(GraphTarget::Modules);
        let idx = crate::index::ensure_ready(&root_path).await?;
        crate::commands::analyze::graph::analyze_dependents(&idx, &target, graph_target)
            .await
            .map_err(|e| format!("Dependents query failed: {}", e))
    }

    /// Trace value provenance for a symbol
    ///
    /// Examples:
    ///   normalize view trace MyFn                # trace value provenance for MyFn
    ///   normalize view trace MyFn --recursive    # recursively trace called functions
    #[cli(display_with = "display_trace")]
    pub async fn trace(
        &self,
        #[param(positional, help = "Symbol to trace (file/symbol or symbol name)")] symbol: String,
        #[param(short = 't', help = "Target file containing the symbol")] target: Option<String>,
        #[param(short = 'd', help = "Maximum depth")] max_depth: Option<usize>,
        #[param(help = "Recursively trace called functions")] recursive: bool,
        #[param(short = 'i', help = "Case-insensitive matching")] case_insensitive: bool,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
    ) -> Result<TraceReport, String> {
        let root_path = Self::root_path(root)?;
        let trace = crate::commands::analyze::trace::build_trace_text(
            &symbol,
            target.as_deref(),
            &root_path,
            max_depth.unwrap_or(50),
            recursive,
            case_insensitive,
        )
        .await?;
        Ok(TraceReport { trace })
    }

    /// Graph-theoretic properties of the dependency graph (requires facts index)
    ///
    /// Examples:
    ///   normalize view graph                     # module dependency graph
    ///   normalize view graph --on symbols        # symbol-level graph
    #[cli(display_with = "display_graph")]
    pub async fn graph(
        &self,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
        #[param(short = 'l', help = "Max examples per section (0=no limit)")] limit: Option<usize>,
        #[param(help = "Graph nodes: modules (default) or symbols")] on: Option<GraphTarget>,
        pretty: bool,
        compact: bool,
    ) -> Result<GraphReport, String> {
        let root_path = Self::root_path(root)?;
        self.resolve_format(pretty, compact, &root_path);
        let effective_limit = match limit.unwrap_or(10) {
            0 => usize::MAX,
            n => n,
        };
        let target = on.unwrap_or(GraphTarget::Modules);
        let idx = crate::index::ensure_ready(&root_path).await?;
        crate::commands::analyze::graph::analyze_graph(&idx, effective_limit, target)
            .await
            .map_err(|e| format!("Graph analysis failed: {}", e))
    }

    /// Git blame with session attribution and code relations
    ///
    /// Examples:
    ///   normalize view blame                     # blame for current directory
    ///   normalize view blame src/                # blame for a subdirectory
    ///   normalize view blame --calls             # include call graph edges
    #[cli(display_with = "display_provenance")]
    #[allow(clippy::too_many_arguments)]
    pub async fn blame(
        &self,
        #[param(positional, help = "Target file or directory scope")] target: Option<String>,
        #[param(help = "Include call graph edges (requires facts index)")] calls: bool,
        #[param(help = "Include co-change edges (from git history)")] coupling: bool,
        #[param(help = "Override session directory")] sessions: Option<String>,
        #[param(short = 'l', help = "Maximum number of files (0=no limit)")] limit: Option<usize>,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
        pretty: bool,
        compact: bool,
    ) -> Result<ProvenanceReport, String> {
        let root_path = Self::root_path(root)?;
        self.resolve_format(pretty, compact, &root_path);
        let effective_limit = match limit.unwrap_or(50) {
            0 => usize::MAX,
            n => n,
        };
        let opts = crate::commands::analyze::provenance::ProvenanceOptions {
            target,
            include_calls: calls,
            include_coupling: coupling,
            sessions_path: sessions,
            limit: effective_limit,
        };
        Ok(crate::commands::analyze::provenance::analyze_provenance(&root_path, &opts).await)
    }
}
