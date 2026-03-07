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

pub mod analyze;
pub mod daemon;
pub mod edit;
pub mod facts;
pub mod generate;
pub mod grammars;
pub mod history;
pub mod package;
pub mod rules;
pub mod serve;
pub mod sessions;
pub mod syntax;
pub mod tools;

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
    /// Text prefix to prepend to view output (used for --dir-context).
    view_prefix: Cell<String>,
    analyze: analyze::AnalyzeService,
    daemon: daemon::DaemonService,
    edit: edit::EditService,
    facts: facts::FactsService,
    grammars: grammars::GrammarService,
    generate: generate::GenerateService,
    package: package::PackageService,
    rules: rules::RulesService,
    serve: serve::ServeService,
    syntax: syntax::SyntaxService,
    sessions: sessions::SessionsService,
    tools: tools::ToolsService,
}

impl Default for NormalizeService {
    fn default() -> Self {
        Self::new()
    }
}

/// Resolve pretty mode from CLI flags and config (TTY auto-detection).
/// Used by all sub-services that receive raw `pretty`/`compact` global flags.
pub(crate) fn resolve_pretty(root: &std::path::Path, pretty: bool, compact: bool) -> bool {
    let config = NormalizeConfig::load(root);
    !compact && (pretty || config.pretty.enabled())
}

impl NormalizeService {
    pub fn new() -> Self {
        let pretty = Cell::new(false);
        Self {
            view_prefix: Cell::new(String::new()),
            analyze: analyze::AnalyzeService::new(&pretty),
            daemon: daemon::DaemonService,
            edit: edit::EditService {
                history: history::HistoryService,
            },
            facts: facts::FactsService::new(&pretty),
            grammars: grammars::GrammarService::new(&pretty),
            generate: generate::GenerateService,
            package: package::PackageService::new(&pretty),
            rules: rules::RulesService::new(&pretty),
            serve: serve::ServeService,
            syntax: syntax::SyntaxService::new(),
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
        self.pretty.set(resolve_pretty(root, pretty, compact));
    }

    /// Generic display bridge that respects pretty/compact state.
    fn display_output<T: OutputFormatter>(&self, value: &T) -> String {
        if self.pretty.get() {
            value.format_pretty()
        } else {
            value.format_text()
        }
    }

    /// Display bridge for GrepResult.
    fn display_grep(&self, value: &GrepResult) -> String {
        self.display_output(value)
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

    /// Display bridge for ViewOutput.
    fn display_view(&self, value: &crate::commands::view::report::ViewOutput) -> String {
        let prefix = self.view_prefix.take();
        let text = self.display_output(value);
        if prefix.is_empty() {
            text
        } else {
            format!("{}{}", prefix, text)
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
    global = [
        pretty = "Human-friendly output with colors and formatting",
        compact = "Compact output without colors (overrides TTY detection)",
    ]
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
        #[param(help = "Prepend directory context (.context.md files)")] dir_context: bool,
        #[param(help = "Exclude paths matching pattern")] exclude: Vec<String>,
        #[param(help = "Include only paths matching pattern")] only: Vec<String>,
        #[param(short = 'i', help = "Case-insensitive symbol matching")] case_insensitive: bool,
        #[param(help = "Show git history for symbol (last N changes)")] history: Option<usize>,
        pretty: bool,
        compact: bool,
    ) -> Result<crate::commands::view::report::ViewOutput, String> {
        let root_path = root
            .map(PathBuf::from)
            .map(Ok)
            .unwrap_or_else(std::env::current_dir)
            .map_err(|e| format!("Failed to get current directory: {e}"))?;

        self.resolve_format(pretty, compact, &root_path);

        let config = NormalizeConfig::load(&root_path);

        let docstring_mode = if no_docs {
            crate::tree::DocstringDisplay::None
        } else if docs || config.view.show_docs() {
            crate::tree::DocstringDisplay::Full
        } else {
            crate::tree::DocstringDisplay::Summary
        };

        // Handle --dir-context: store prefix so display_view can prepend it
        if dir_context {
            let target_path = target
                .as_ref()
                .map(|t| root_path.join(t))
                .unwrap_or_else(|| root_path.clone());
            if let Some(ctx) =
                crate::commands::context::get_merged_context(&root_path, &target_path)
            {
                self.view_prefix.set(format!("{}\n\n---\n\n", ctx));
            }
        }

        crate::commands::view::build_view_service(
            target.as_deref(),
            &root_path,
            depth,
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
            history,
        )
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
            .map(Ok)
            .unwrap_or_else(std::env::current_dir)
            .map_err(|e| format!("Failed to get current directory: {e}"))?;

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
            .map(Ok)
            .unwrap_or_else(std::env::current_dir)
            .map_err(|e| format!("Failed to get current directory: {e}"))?;

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
            .map(Ok)
            .unwrap_or_else(std::env::current_dir)
            .map_err(|e| format!("Failed to get current directory: {e}"))?;
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
    pub async fn init(
        &self,
        #[param(help = "Index the codebase after initialization")] index: bool,
        #[param(help = "Run interactive rule setup wizard after initialization")] setup: bool,
    ) -> Result<InitResult, String> {
        use std::fs;

        let root = std::env::current_dir()
            .map_err(|e| format!("Failed to get current directory: {}", e))?;
        let mut changes = Vec::new();

        // 1. Create .normalize directory if needed
        let normalize_dir = root.join(".normalize");
        if !normalize_dir.exists() {
            fs::create_dir_all(&normalize_dir)
                .map_err(|e| format!("Failed to create .normalize directory: {}", e))?;
            changes.push("Created .normalize/".to_string());
        }

        // 2. Detect TODO files for alias config
        let todo_files = commands::init::detect_todo_files(&root);

        // 3. Create config.toml if missing
        let config_path = normalize_dir.join("config.toml");
        if !config_path.exists() {
            let aliases_section = if todo_files.is_empty() {
                String::new()
            } else {
                let files_str = todo_files
                    .iter()
                    .map(|f| format!("\"{}\"", f))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("\n[aliases]\ntodo = [{}]\n", files_str)
            };

            let default_config = format!(
                r#"# Normalize configuration
# See: https://github.com/rhi-zone/normalize

[daemon]
# enabled = true
# auto_start = true

[analyze]
# clones = true

# [analyze.weights]
# health = 1.0
# complexity = 0.5
# security = 2.0
# clones = 0.3
{}"#,
                aliases_section
            );
            fs::write(&config_path, default_config)
                .map_err(|e| format!("Failed to create config.toml: {}", e))?;
            changes.push("Created .normalize/config.toml".to_string());
            for f in &todo_files {
                changes.push(format!("Detected TODO file: {}", f));
            }
        }

        // 4. Update .gitignore if needed
        let gitignore_path = root.join(".gitignore");
        let gitignore_changes = commands::init::update_gitignore(&gitignore_path);
        changes.extend(gitignore_changes);

        // 5. Report changes
        if changes.is_empty() {
            println!("Already initialized.");
        } else {
            println!("Initialized normalize:");
            for change in &changes {
                println!("  {}", change);
            }
        }

        // 6. Optionally index
        if index {
            println!("\nIndexing codebase...");
            let mut idx = crate::index::open(&root)
                .await
                .map_err(|e| format!("Failed to open index: {}", e))?;
            let count = idx
                .refresh()
                .await
                .map_err(|e| format!("Failed to index: {}", e))?;
            println!("Indexed {} files.", count);
        }

        // 7. Optionally run setup wizard
        if setup {
            commands::init::cmd_setup_wizard(&root);
        }

        Ok(InitResult {
            success: true,
            message: if changes.is_empty() {
                "Already initialized.".to_string()
            } else {
                "Initialization complete.".to_string()
            },
        })
    }

    /// Check for and install updates
    pub fn update(
        &self,
        #[param(short = 'c', help = "Check for updates without installing")] check: bool,
    ) -> Result<UpdateResult, String> {
        use std::io::Read;

        const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");
        const GITHUB_REPO: &str = "rhi-zone/normalize";

        let client = ureq::agent();

        let url = format!(
            "https://api.github.com/repos/{}/releases/latest",
            GITHUB_REPO
        );

        let response = client
            .get(&url)
            .set("User-Agent", "normalize-cli")
            .set("Accept", "application/vnd.github+json")
            .call()
            .map_err(|e| format!("Failed to check for updates: {}", e))?;

        let body: serde_json::Value = response
            .into_json()
            .map_err(|e| format!("Failed to parse response: {}", e))?;

        let latest_version = body["tag_name"]
            .as_str()
            .unwrap_or("unknown")
            .trim_start_matches('v')
            .to_string();

        let is_update_available = latest_version != CURRENT_VERSION
            && commands::update::version_gt(&latest_version, CURRENT_VERSION);

        if check || !is_update_available {
            return Ok(UpdateResult {
                current_version: CURRENT_VERSION.to_string(),
                latest_version,
                update_available: is_update_available,
                message: None,
            });
        }

        // Perform the update
        eprintln!("Downloading update...");

        let target = commands::update::get_target_triple();
        let asset_name = commands::update::get_asset_name(&target);

        let assets = body["assets"].as_array();
        let asset_url = assets
            .and_then(|arr| {
                arr.iter()
                    .find(|a| a["name"].as_str() == Some(&asset_name))
                    .and_then(|a| a["browser_download_url"].as_str())
            })
            .ok_or_else(|| format!("No binary available for your platform: {}", target))?;

        eprintln!("  Downloading {}...", asset_name);
        let archive_response = client
            .get(asset_url)
            .call()
            .map_err(|e| format!("Failed to download update: {}", e))?;

        let mut archive_data = Vec::new();
        archive_response
            .into_reader()
            .read_to_end(&mut archive_data)
            .map_err(|e| format!("Failed to read download: {}", e))?;

        // Checksum verification
        let checksum_url = assets.and_then(|arr| {
            arr.iter()
                .find(|a| a["name"].as_str() == Some("SHA256SUMS.txt"))
                .and_then(|a| a["browser_download_url"].as_str())
        });

        if let Some(checksum_url) = checksum_url {
            eprintln!("  Verifying checksum...");
            if let Ok(resp) = client.get(checksum_url).call()
                && let Ok(checksums) = resp.into_string()
            {
                let expected = checksums
                    .lines()
                    .find(|line| line.contains(&asset_name))
                    .and_then(|line| line.split_whitespace().next());

                if let Some(expected) = expected {
                    let actual = commands::update::sha256_hex(&archive_data);
                    if actual != expected {
                        return Err(format!(
                            "Checksum mismatch!\n  Expected: {}\n  Got:      {}",
                            expected, actual
                        ));
                    }
                }
            }
        }

        // Extract binary
        eprintln!("  Extracting...");
        let binary_data = if asset_name.ends_with(".tar.gz") {
            commands::update::extract_tar_gz(&archive_data)
        } else if asset_name.ends_with(".zip") {
            commands::update::extract_zip(&archive_data)
        } else {
            Err(format!("Unknown archive format: {}", asset_name))
        }?;

        // Replace current binary
        eprintln!("  Installing...");
        commands::update::self_replace(&binary_data)?;

        Ok(UpdateResult {
            current_version: CURRENT_VERSION.to_string(),
            latest_version,
            update_available: true,
            message: Some(
                "Updated successfully! Restart normalize to use the new version.".to_string(),
            ),
        })
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
        use commands::translate::{SourceLanguage, TargetLanguage};

        let to_lang: TargetLanguage = to.parse().map_err(|e: String| e)?;
        let from_lang: Option<SourceLanguage> =
            from.map(|s| s.parse().map_err(|e: String| e)).transpose()?;

        let is_stdin = input == "-";
        let input_path = std::path::PathBuf::from(&input);

        // Read input (file or stdin)
        let content = if is_stdin {
            use std::io::Read;
            let mut buf = String::new();
            std::io::stdin()
                .read_to_string(&mut buf)
                .map_err(|e| format!("Failed to read stdin: {}", e))?;
            buf
        } else {
            std::fs::read_to_string(&input_path)
                .map_err(|e| format!("Failed to read {}: {}", input, e))?
        };

        // Determine source language
        let source_lang = match from_lang {
            Some(lang) => lang.as_str(),
            None => {
                if is_stdin {
                    return Err("--from is required when reading from stdin".to_string());
                }
                match input_path.extension().and_then(|e| e.to_str()) {
                    Some("ts") | Some("tsx") | Some("js") | Some("jsx") => "typescript",
                    Some("lua") => "lua",
                    Some("py") => "python",
                    _ => {
                        return Err(
                            "Cannot detect language from extension. Use --from to specify source language."
                                .to_string(),
                        );
                    }
                }
            }
        };

        let reader = normalize_surface_syntax::registry::reader_for_language(source_lang)
            .ok_or_else(|| format!("No reader available for language: {}", source_lang))?;

        let target_lang = to_lang.as_str();
        let writer = normalize_surface_syntax::registry::writer_for_language(target_lang)
            .ok_or_else(|| format!("No writer available for language: {}", target_lang))?;

        let ir = reader
            .read(&content)
            .map_err(|e| format!("Failed to parse {} as {}: {}", input, source_lang, e))?;

        let code = writer.write(&ir);

        if let Some(ref path) = output {
            std::fs::write(path, &code).map_err(|e| format!("Failed to write {}: {}", path, e))?;
            eprintln!("Translated {} -> {} ({})", input, path, target_lang);
            Ok(TranslateResult {
                code,
                source_language: source_lang.to_string(),
                target_language: target_lang.to_string(),
                input_path: input,
                output_path: Some(path.clone()),
            })
        } else {
            Ok(TranslateResult {
                code,
                source_language: source_lang.to_string(),
                target_language: target_lang.to_string(),
                input_path: input,
                output_path: None,
            })
        }
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

    /// AST inspection and syntax rules
    pub fn syntax(&self) -> &syntax::SyntaxService {
        &self.syntax
    }

    /// Package management: info, list, tree, outdated
    pub fn package(&self) -> &package::PackageService {
        &self.package
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

    /// Analyze codebase (health, complexity, security, duplicates, docs)
    pub fn analyze(&self) -> &analyze::AnalyzeService {
        &self.analyze
    }

    /// Manage and run syntax/fact rules
    pub fn rules(&self) -> &rules::RulesService {
        &self.rules
    }

    /// Start a normalize server (MCP, HTTP, LSP)
    pub fn serve(&self) -> &serve::ServeService {
        &self.serve
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
