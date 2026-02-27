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
//! server-less uses `Display` for default text output and handles `--json`,
//! `--jsonl`, `--jq` automatically via `Serialize`. Types implementing
//! `OutputFormatter` get `Display` via `format_text()`.
//!
//! TODO: Once server-less `display_with` supports JSON flag passthrough,
//! switch to `display_with` for `--pretty`/`--compact` toggle.

use crate::commands;
use crate::config::NormalizeConfig;
use crate::output::OutputFormatter;
use crate::text_search::{self, GrepResult};
use server_less::cli;
use std::path::PathBuf;

/// Root CLI service for normalize.
#[derive(Default)]
pub struct NormalizeService;

impl NormalizeService {
    pub fn new() -> Self {
        Self
    }

    /// Provide config-based defaults for parameters.
    ///
    /// Called by server-less when a required parameter is not provided on the CLI.
    /// Loads config from the current directory (or --root if available) and returns
    /// the config value as a string.
    fn config_defaults(&self, param: &str) -> Option<String> {
        let config = NormalizeConfig::load(&std::env::current_dir().unwrap_or_default());
        match param {
            "limit" => Some(config.text_search.limit().to_string()),
            _ => None,
        }
    }
}

#[cli(
    name = "normalize",
    version = "0.1.0",
    about = "Fast code intelligence CLI",
    defaults = "config_defaults"
)]
impl NormalizeService {
    /// Search for text patterns in files (fast ripgrep-based search)
    pub fn grep(
        &self,
        #[param(help = "Regex pattern to search for")] pattern: String,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
        #[param(short = 'l', help = "Maximum number of matches to return")] limit: Option<usize>,
        #[param(short = 'i', help = "Case-insensitive search")] ignore_case: bool,
        #[param(help = "Exclude files matching patterns or aliases (comma-separated)")]
        exclude: Option<String>,
        #[param(help = "Only include files matching patterns or aliases (comma-separated)")]
        only: Option<String>,
    ) -> Result<GrepResult, String> {
        let root_path = root
            .map(PathBuf::from)
            .unwrap_or_else(|| std::env::current_dir().unwrap());
        let config = NormalizeConfig::load(&root_path);

        let limit = limit.unwrap_or_else(|| config.text_search.limit());
        let ignore_case = ignore_case || config.text_search.ignore_case();

        let exclude_list: Vec<String> = exclude
            .map(|s| s.split(',').map(|s| s.trim().to_string()).collect())
            .unwrap_or_default();
        let only_list: Vec<String> = only
            .map(|s| s.split(',').map(|s| s.trim().to_string()).collect())
            .unwrap_or_default();

        let filter = commands::build_filter(&root_path, &exclude_list, &only_list);

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
}

/// Display impl bridges to OutputFormatter::format_text() for server-less CLI output.
impl std::fmt::Display for GrepResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Trim trailing newline from format_text since Display doesn't add one
        let text = self.format_text();
        write!(f, "{}", text.trim_end())
    }
}
