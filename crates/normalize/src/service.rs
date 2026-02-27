//! Server-less `#[cli]` service layer for normalize.
//!
//! NormalizeService wraps the existing command implementations behind
//! server-less's `#[cli]` macro, which generates CLI parsing, JSON output,
//! schema introspection, and params-json support from method signatures.
//!
//! ## Migration status
//!
//! Commands are migrated incrementally from the legacy clap dispatch in main.rs.
//! During migration, both paths coexist: server-less handles migrated commands,
//! legacy dispatch handles the rest.
//!
//! ## Output formatting
//!
//! server-less handles `--json`/`--jsonl`/`--jq` automatically via `Serialize`.
//! For text output, `display_with` bridges to `OutputFormatter`: each method's
//! `display_output` reads `self.pretty` (set by the method from `--pretty`/
//! `--compact` globals + config) and calls `format_pretty()` or `format_text()`.

use crate::commands;
use crate::commands::aliases::{AliasesReport, detect_project_languages};
use crate::commands::context::{ContextListReport, ContextReport, collect_context_files};
use crate::config::NormalizeConfig;
use crate::output::OutputFormatter;
use crate::text_search::{self, GrepResult};
use server_less::cli;
use std::cell::Cell;
use std::path::PathBuf;

/// Root CLI service for normalize.
pub struct NormalizeService {
    /// Whether pretty output is active (resolved per-command from globals + config).
    pretty: Cell<bool>,
}

impl Default for NormalizeService {
    fn default() -> Self {
        Self::new()
    }
}

impl NormalizeService {
    pub fn new() -> Self {
        Self {
            pretty: Cell::new(false),
        }
    }

    /// Provide config-based defaults for parameters.
    ///
    /// Called by server-less when a required parameter is not provided on the CLI.
    /// Loads config from the current directory and returns the config value as a string.
    fn config_defaults(&self, param: &str) -> Option<String> {
        let config = NormalizeConfig::load(&std::env::current_dir().unwrap_or_default());
        match param {
            "limit" => Some(config.text_search.limit().to_string()),
            _ => None,
        }
    }

    /// Resolve pretty/compact state from globals and config, store in `self.pretty`.
    fn resolve_format(&self, pretty: bool, compact: bool, root: &std::path::Path) {
        let config = NormalizeConfig::load(root);
        let is_pretty = !compact && (pretty || config.pretty.enabled());
        self.pretty.set(is_pretty);
    }

    /// Display bridge for GrepResult.
    fn display_grep(&self, value: &GrepResult) -> String {
        if self.pretty.get() {
            value.format_pretty()
        } else {
            value.format_text()
        }
    }

    /// Display bridge for ContextOutput (dispatches to inner type).
    fn display_context(&self, value: &ContextOutput) -> String {
        match value {
            ContextOutput::List(r) => r.format_text(),
            ContextOutput::Full(r) => r.format_text(),
        }
    }
}

/// Wrapper enum for context command's two output types.
#[derive(serde::Serialize, schemars::JsonSchema)]
#[serde(untagged)]
pub enum ContextOutput {
    List(ContextListReport),
    Full(ContextReport),
}

/// Init command result.
#[derive(serde::Serialize, schemars::JsonSchema)]
pub struct InitResult {
    pub success: bool,
    pub message: String,
}

#[cli(
    name = "normalize",
    version = "0.1.0",
    about = "Fast code intelligence CLI",
    defaults = "config_defaults",
    global = [pretty, compact]
)]
impl NormalizeService {
    /// Search for text patterns in files (fast ripgrep-based search)
    #[cli(display_with = "display_grep")]
    #[allow(clippy::too_many_arguments)]
    pub fn grep(
        &self,
        #[param(positional, help = "Regex pattern to search for")] pattern: String,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
        #[param(short = 'l', help = "Maximum number of matches to return")] limit: Option<usize>,
        #[param(short = 'i', help = "Case-insensitive search")] ignore_case: bool,
        #[param(help = "Exclude files matching patterns or aliases")] exclude: Vec<String>,
        #[param(help = "Only include files matching patterns or aliases")] only: Vec<String>,
        pretty: bool,
        compact: bool,
    ) -> Result<GrepResult, String> {
        let root_path = root
            .map(PathBuf::from)
            .unwrap_or_else(|| std::env::current_dir().unwrap());

        self.resolve_format(pretty, compact, &root_path);

        let config = NormalizeConfig::load(&root_path);
        let limit = limit.unwrap_or_else(|| config.text_search.limit());
        let ignore_case = ignore_case || config.text_search.ignore_case();

        let filter = commands::build_filter(&root_path, &exclude, &only);

        match text_search::grep(&pattern, &root_path, filter.as_ref(), limit, ignore_case) {
            Ok(result) => {
                if result.matches.is_empty() {
                    return Err(format!("No matches found for: {}", pattern));
                }
                Ok(result)
            }
            Err(e) => Err(format!("Error: {}", e)),
        }
    }

    /// List filter aliases (used by --exclude/--only)
    pub fn aliases(
        &self,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
    ) -> Result<AliasesReport, String> {
        let root_path = root
            .map(PathBuf::from)
            .unwrap_or_else(|| std::env::current_dir().unwrap());

        let config = NormalizeConfig::load(&root_path);
        let languages = detect_project_languages(&root_path);

        Ok(AliasesReport::build(&config, &languages))
    }

    /// Show directory context (hierarchical .context.md files)
    #[cli(display_with = "display_context")]
    pub fn context(
        &self,
        #[param(positional, help = "Target path to collect context for")] target: Option<String>,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
        #[param(help = "Show only file paths, not content")] list: bool,
    ) -> Result<ContextOutput, String> {
        let root_path = root
            .map(PathBuf::from)
            .unwrap_or_else(|| std::env::current_dir().unwrap());
        let target_str = target.as_deref().unwrap_or(".");
        let target = root_path.join(target_str);

        let target_dir = if target.is_file() {
            target.parent().unwrap_or(&root_path).to_path_buf()
        } else {
            target.clone()
        };

        let root_canon = root_path
            .canonicalize()
            .map_err(|e| format!("Failed to resolve root: {}", e))?;
        let target_canon = target_dir
            .canonicalize()
            .map_err(|e| format!("Failed to resolve target: {}", e))?;

        let files = collect_context_files(&root_canon, &target_canon);

        if list {
            let paths: Vec<String> = files
                .iter()
                .map(|f| f.to_str().unwrap_or("").to_string())
                .collect();
            Ok(ContextOutput::List(ContextListReport::new(paths)))
        } else {
            let entries = files
                .iter()
                .map(|file| {
                    let rel_path = file.strip_prefix(&root_canon).unwrap_or(file);
                    let content = std::fs::read_to_string(file).unwrap_or_default();
                    (rel_path.display().to_string(), content)
                })
                .collect();
            Ok(ContextOutput::Full(ContextReport::new(entries)))
        }
    }

    /// Initialize normalize in current directory
    pub fn init(
        &self,
        #[param(help = "Index the codebase after initialization")] index: bool,
    ) -> Result<InitResult, String> {
        let root = std::env::current_dir()
            .map_err(|e| format!("Failed to get current directory: {}", e))?;
        let exit_code = commands::init::cmd_init(&root, index);
        if exit_code == 0 {
            Ok(InitResult {
                success: true,
                message: "Initialization complete.".to_string(),
            })
        } else {
            Err("Initialization failed.".to_string())
        }
    }
}

/// Display impl bridges to OutputFormatter::format_text() for contexts outside
/// server-less dispatch (e.g. direct use of GrepResult).
impl std::fmt::Display for GrepResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.format_text().trim_end())
    }
}

impl std::fmt::Display for AliasesReport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.format_text().trim_end())
    }
}

impl std::fmt::Display for ContextOutput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ContextOutput::List(r) => write!(f, "{}", r.format_text().trim_end()),
            ContextOutput::Full(r) => write!(f, "{}", r.format_text().trim_end()),
        }
    }
}

impl std::fmt::Display for InitResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}
