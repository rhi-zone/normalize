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

pub mod daemon;
pub mod edit;
pub mod facts;
pub mod generate;
pub mod grammars;
pub mod history;
pub mod package;
pub mod rules;
pub mod sessions;
pub mod tools;

use crate::commands;
use crate::commands::aliases::{AliasesReport, detect_project_languages};
use crate::commands::context::{ContextListReport, ContextReport, collect_context_files};
use crate::commands::translate::{SourceLanguage, TargetLanguage};
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
    daemon: daemon::DaemonService,
    edit: edit::EditService,
    facts: facts::FactsService,
    grammars: grammars::GrammarService,
    generate: generate::GenerateService,
    history: history::HistoryService,
    package: package::PackageService,
    rules: rules::RulesService,
    sessions: sessions::SessionsService,
    tools: tools::ToolsService,
}

impl Default for NormalizeService {
    fn default() -> Self {
        Self::new()
    }
}

impl NormalizeService {
    pub fn new() -> Self {
        let pretty = Cell::new(false);
        Self {
            daemon: daemon::DaemonService,
            edit: edit::EditService,
            facts: facts::FactsService::new(&pretty),
            grammars: grammars::GrammarService::new(&pretty),
            generate: generate::GenerateService,
            history: history::HistoryService,
            package: package::PackageService::new(&pretty),
            rules: rules::RulesService::new(&pretty),
            sessions: sessions::SessionsService::new(&pretty),
            tools: tools::ToolsService::new(),
            pretty,
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
            "depth" => Some(config.view.depth().to_string()),
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

    /// Display bridge for TranslateResult.
    fn display_translate(&self, value: &TranslateResult) -> String {
        if let Some(ref path) = value.output_path {
            format!(
                "Translated {} -> {} ({})",
                value.input_path, path, value.target_language
            )
        } else {
            value.code.clone()
        }
    }

    /// Display bridge for ViewResult.
    fn display_view(&self, value: &crate::commands::view::report::ViewResult) -> String {
        value.text.clone()
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

/// Update check result.
#[derive(serde::Serialize, schemars::JsonSchema)]
pub struct UpdateResult {
    pub current_version: String,
    pub latest_version: String,
    pub update_available: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

impl std::fmt::Display for UpdateResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Current version: {}", self.current_version)?;
        writeln!(f, "Latest version:  {}", self.latest_version)?;
        if let Some(ref msg) = self.message {
            write!(f, "{}", msg)?;
        } else if self.update_available {
            write!(f, "\nUpdate available! Run 'normalize update' to install.")?;
        } else {
            write!(f, "You are running the latest version.")?;
        }
        Ok(())
    }
}

/// Translate command result.
#[derive(serde::Serialize, schemars::JsonSchema)]
pub struct TranslateResult {
    pub code: String,
    pub source_language: String,
    pub target_language: String,
    pub input_path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_path: Option<String>,
}

impl std::fmt::Display for TranslateResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.output_path.is_some() {
            // File was written, show nothing on stdout (message went to stderr)
            Ok(())
        } else {
            write!(f, "{}", self.code)
        }
    }
}

#[cli(
    name = "normalize",
    version = "0.1.0",
    about = "Fast code intelligence CLI",
    defaults = "config_defaults",
    global = [pretty, compact]
)]
impl NormalizeService {
    /// View a node in the codebase tree (directory, file, or symbol)
    #[cli(display_with = "display_view")]
    #[allow(clippy::too_many_arguments)]
    pub fn view(
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
        depth: i32,
        #[param(short = 'n', help = "Show line numbers")] line_numbers: bool,
        #[param(help = "Show dependencies (imports/exports)")] deps: bool,
        #[param(short = 'k', help = "Filter by symbol kind: class, function, method")] kind: Option<
            String,
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
        #[param(help = "Prepend directory context (.context.md files)")] dir_context: bool,
        #[param(help = "Exclude paths matching pattern")] exclude: Vec<String>,
        #[param(help = "Include only paths matching pattern")] only: Vec<String>,
        #[param(short = 'i', help = "Case-insensitive symbol matching")] case_insensitive: bool,
        #[param(help = "Show git history for symbol (last N changes)")] history: Option<usize>,
        pretty: bool,
        compact: bool,
    ) -> Result<crate::commands::view::report::ViewResult, String> {
        let root_path = root
            .map(PathBuf::from)
            .unwrap_or_else(|| std::env::current_dir().unwrap());

        self.resolve_format(pretty, compact, &root_path);

        let config = NormalizeConfig::load(&root_path);

        let docstring_mode = if no_docs {
            crate::tree::DocstringDisplay::None
        } else if docs || config.view.show_docs() {
            crate::tree::DocstringDisplay::Full
        } else {
            crate::tree::DocstringDisplay::Summary
        };

        // Handle --dir-context: prepend directory context to text output
        let prefix = if dir_context {
            let target_path = target
                .as_ref()
                .map(|t| root_path.join(t))
                .unwrap_or_else(|| root_path.clone());
            crate::commands::context::get_merged_context(&root_path, &target_path)
                .map(|ctx| format!("{}\n\n---\n\n", ctx))
        } else {
            None
        };

        let mut result = crate::commands::view::build_view_service(
            target.as_deref(),
            &root_path,
            depth,
            line_numbers,
            deps,
            kind.as_deref(),
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
            history,
            self.pretty.get(),
        )?;

        if let Some(prefix_text) = prefix {
            result.text = format!("{}{}", prefix_text, result.text);
        }

        Ok(result)
    }

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

    /// Check for and install updates
    pub fn update(
        &self,
        #[param(short = 'c', help = "Check for updates without installing")] check: bool,
    ) -> Result<UpdateResult, String> {
        commands::update::cmd_update_service(check)
    }

    /// Translate code between programming languages
    #[cli(display_with = "display_translate")]
    pub fn translate(
        &self,
        #[param(positional, help = "Input source file, use - for stdin")] input: String,
        #[param(short = 't', help = "Target language")] to: String,
        #[param(
            short = 'f',
            help = "Source language (auto-detect from extension if omitted)"
        )]
        from: Option<String>,
        #[param(short = 'o', help = "Output file (stdout if not specified)")] output: Option<
            String,
        >,
    ) -> Result<TranslateResult, String> {
        let to_lang: TargetLanguage = to.parse().map_err(|e: String| e)?;
        let from_lang: Option<SourceLanguage> =
            from.map(|s| s.parse().map_err(|e: String| e)).transpose()?;

        commands::translate::cmd_translate_service(&input, from_lang, to_lang, output.as_deref())
    }

    /// Manage the global normalize daemon
    pub fn daemon(&self) -> &daemon::DaemonService {
        &self.daemon
    }

    /// Manage tree-sitter grammars for parsing
    pub fn grammars(&self) -> &grammars::GrammarService {
        &self.grammars
    }

    /// Generate code from API spec
    pub fn generate(&self) -> &generate::GenerateService {
        &self.generate
    }

    /// Extract and query code facts (symbols, imports, calls)
    pub fn facts(&self) -> &facts::FactsService {
        &self.facts
    }

    /// Manage and run analysis rules (syntax + fact)
    pub fn rules(&self) -> &rules::RulesService {
        &self.rules
    }

    /// Package management: info, list, tree, outdated
    pub fn package(&self) -> &package::PackageService {
        &self.package
    }

    /// View shadow git edit history
    pub fn history(&self) -> &history::HistoryService {
        &self.history
    }

    /// Analyze agent session logs (Claude Code, Codex, Gemini)
    pub fn sessions(&self) -> &sessions::SessionsService {
        &self.sessions
    }

    /// External ecosystem tools (linters, formatters, test runners)
    pub fn tools(&self) -> &tools::ToolsService {
        &self.tools
    }

    /// Structural editing of code symbols
    pub fn edit(&self) -> &edit::EditService {
        &self.edit
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
