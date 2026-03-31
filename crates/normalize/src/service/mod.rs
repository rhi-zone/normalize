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
pub mod config;
pub mod daemon;
pub mod edit;
pub mod facts;
pub mod generate;
pub mod grammars;
pub mod guide;
pub mod history;
pub mod package;
pub mod rank;
pub mod ratchet;
pub mod rename;
// rules module moved to normalize-rules crate; re-exported for internal use
pub mod serve;
pub mod sessions;
pub mod syntax;
pub mod tools;
pub mod trend;
pub mod view;

use crate::commands;
use crate::commands::aliases::{AliasesReport, detect_project_languages};
use crate::commands::context::{
    CallerContext, ContextBlock, ContextListReport, ContextReport, collect_new_context_files,
    parse_match_pairs, read_stdin_context, resolve_context, yaml_to_json,
};
use crate::config::NormalizeConfig;
use crate::output::OutputFormatter;
use crate::text_search::{self, GrepReport};
use server_less::cli;
use std::cell::Cell;
use std::path::PathBuf;

/// Root CLI service for normalize.
pub struct NormalizeService {
    /// Whether pretty output is active (resolved per-command from globals + config).
    pretty: Cell<bool>,
    analyze: analyze::AnalyzeService,
    config: config::ConfigService,
    daemon: daemon::DaemonService,
    edit: edit::EditService,
    structure: facts::FactsService,
    grammars: grammars::GrammarService,
    guide: guide::GuideService,
    generate: generate::GenerateService,
    package: package::PackageService,
    rank: rank::RankService,
    budget: normalize_budget::service::BudgetService,
    ratchet: normalize_ratchet::service::RatchetService,
    rules: normalize_rules::RulesService,
    serve: serve::ServeService,
    syntax: syntax::SyntaxService,
    sessions: sessions::SessionsService,
    tools: tools::ToolsService,
    trend: trend::TrendService,
    view: view::ViewService,
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
    /// Construct a new `NormalizeService` with all sub-services initialized.
    ///
    /// Creates each sub-service (analyze, rank, view, facts, rules, etc.) sharing a single
    /// `pretty` cell that is updated per-command from global `--pretty`/`--compact` flags.
    /// Called once at startup by the CLI entry point.
    pub fn new() -> Self {
        let pretty = Cell::new(false);
        Self {
            analyze: analyze::AnalyzeService::new(&pretty),
            config: config::ConfigService::new(&pretty),
            daemon: daemon::DaemonService,
            edit: edit::EditService {
                history: history::HistoryService,
            },
            structure: facts::FactsService::new(&pretty),
            grammars: grammars::GrammarService::new(&pretty),
            guide: guide::GuideService,
            generate: generate::GenerateService,
            package: package::PackageService::new(&pretty),
            rank: rank::RankService::new(&pretty),
            budget: normalize_budget::service::BudgetService::new(pretty.get()),
            ratchet: normalize_ratchet::service::RatchetService::new(pretty.get()),
            rules: normalize_rules::RulesService::new(&pretty),
            serve: serve::ServeService,
            syntax: syntax::SyntaxService::new(),
            sessions: sessions::SessionsService::new(&pretty),
            tools: tools::ToolsService::new(),
            trend: trend::TrendService::new(&pretty),
            view: view::ViewService::new(&pretty),
            pretty,
        }
    }

    /// Provide config-based defaults for parameters.
    ///
    /// Called by server-less when a required parameter is not provided on the CLI.
    /// Loads config from the current directory and returns the config value as a string.
    fn config_defaults(&self, param: &str) -> Option<String> {
        let config = NormalizeConfig::load(
            &std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from(".")),
        );
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

    /// Display bridge for ContextKindReport (dispatches to inner type).
    fn display_context(&self, value: &ContextKindReport) -> String {
        let pretty = self.pretty.get();
        match value {
            ContextKindReport::List(r) => r.format_text(),
            ContextKindReport::Full(r) => {
                if pretty {
                    r.format_pretty()
                } else {
                    r.format_text()
                }
            }
        }
    }

    /// Display bridge for TranslateReport.
    fn display_translate(&self, value: &TranslateReport) -> String {
        if let Some(ref path) = value.output_path {
            format!(
                "Translated {} -> {} ({})",
                value.input_path, path, value.target_language
            )
        } else {
            value.code.clone()
        }
    }
}

/// Output type for `normalize context`: either a list of context files or full content.
#[derive(serde::Serialize, schemars::JsonSchema)]
#[serde(tag = "kind")]
pub enum ContextKindReport {
    List(ContextListReport),
    Full(ContextReport),
}

impl OutputFormatter for ContextKindReport {
    fn format_text(&self) -> String {
        match self {
            Self::List(r) => r.format_text(),
            Self::Full(r) => r.format_text(),
        }
    }

    fn format_pretty(&self) -> String {
        match self {
            Self::List(r) => r.format_text(),
            Self::Full(r) => r.format_pretty(),
        }
    }
}

/// Report for `normalize init`: records what changed during initialization.
#[derive(serde::Serialize, schemars::JsonSchema)]
pub struct InitReport {
    pub message: String,
    pub changes: Vec<String>,
    pub dry_run: bool,
}

/// Report for `normalize update`: current and latest version with an update-available flag.
#[derive(serde::Serialize, schemars::JsonSchema)]
pub struct UpdateReport {
    pub current_version: String,
    pub latest_version: String,
    pub update_available: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

impl OutputFormatter for UpdateReport {
    fn format_text(&self) -> String {
        use std::fmt::Write as _;
        let mut out = String::new();
        let _ = writeln!(out, "Current version: {}", self.current_version);
        let _ = writeln!(out, "Latest version:  {}", self.latest_version);
        if let Some(ref msg) = self.message {
            let _ = write!(out, "{}", msg);
        } else if self.update_available {
            let _ = write!(
                out,
                "\nUpdate available! Run 'normalize update' to install."
            );
        } else {
            let _ = write!(out, "You are running the latest version.");
        }
        out
    }
}

/// Report for `normalize translate`: translated code and optional output path.
///
/// When `output_path` is set, the translated code was written to disk and `format_text()`
/// returns an empty string (the write message was sent to stderr). When absent, the
/// translated code is printed to stdout via `code`.
#[derive(serde::Serialize, schemars::JsonSchema)]
pub struct TranslateReport {
    pub code: String,
    pub source_language: String,
    pub target_language: String,
    pub input_path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_path: Option<String>,
}

impl OutputFormatter for TranslateReport {
    fn format_text(&self) -> String {
        if self.output_path.is_some() {
            // File was written, show nothing on stdout (message went to stderr)
            String::new()
        } else {
            self.code.clone()
        }
    }
}

#[cli(
    name = "normalize",
    description = "Structural code intelligence: index symbols and calls, enforce rules, track complexity.",
    defaults = "config_defaults",
    global = [
        pretty = "Human-friendly output with colors and formatting",
        compact = "Compact output without colors (overrides TTY detection)",
    ]
)]
impl NormalizeService {
    /// View a node in the codebase tree, or navigate symbol relationships
    pub fn view(&self) -> &view::ViewService {
        &self.view
    }

    /// Search for text patterns across the codebase using ripgrep regex syntax.
    ///
    /// Accepts a regex `pattern`, optional positional `path` (or `--root`) for directory scoping,
    /// `only` (include glob), `exclude` (exclude glob), and `limit` flags. Returns a `GrepReport`
    /// with file paths, line numbers, and matched text. Uses ripgrep regex: `|` for alternation,
    /// not BRE/ERE. When both `path` and `--root` are provided, `path` wins.
    ///
    /// Examples:
    ///   normalize grep "TODO" --only "*.rs"    # search Rust files for TODO
    ///   normalize grep "fn main" src/          # search in specific directory
    ///   normalize grep "class \w+" --only "*.py" --json   # JSON output
    #[cli(display_with = "display_output")]
    #[allow(clippy::too_many_arguments)]
    pub fn grep(
        &self,
        #[param(positional, help = "Regex pattern to search for")] pattern: String,
        #[param(positional, help = "Directory to search in (overrides --root)")] path: Option<
            String,
        >,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
        #[param(short = 'l', help = "Maximum number of matches to return")] limit: Option<usize>,
        #[param(short = 'i', help = "Case-insensitive search")] ignore_case: bool,
        #[param(help = "Exclude files matching patterns or aliases")] exclude: Vec<String>,
        #[param(help = "Only include files matching patterns or aliases")] only: Vec<String>,
        pretty: bool,
        compact: bool,
    ) -> Result<GrepReport, String> {
        // `path` positional takes precedence over `--root` flag.
        let root_path = path
            .or(root)
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
    ///
    /// Examples:
    ///   normalize aliases                      # list all filter aliases
    #[cli(display_with = "display_output")]
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

    /// Resolve contextual text from .normalize/context/ hierarchy
    ///
    /// Walks .normalize/context/ directories bottom-up from the working directory
    /// (project-specific first, global ~/.normalize/context/ last). Each .md file
    /// may contain YAML frontmatter; blocks are filtered by matching the frontmatter
    /// against caller-provided context (--match / --stdin).
    ///
    /// Without conditions and no matching frontmatter keys → block always matches.
    ///
    /// Examples:
    ///   normalize context                                          # dump all (no filter)
    ///   normalize context --match hook=UserPromptSubmit            # filter by key=value
    ///   normalize context --match claudecode.hook=UserPromptSubmit # nested dot-path
    ///   echo '{"hook":"X"}' | normalize context --stdin --prefix claudecode
    ///   normalize context --all --list                            # list all source files
    #[cli(display_with = "display_context")]
    #[allow(clippy::too_many_arguments)]
    pub fn context(
        &self,
        #[param(help = "Root directory for hierarchy walk (default: cwd)")] root: Option<String>,
        #[param(help = "Match context against KEY=VALUE pair (repeatable)")] r#match: Vec<String>,
        #[param(help = "Read context JSON from stdin")] stdin: bool,
        #[param(help = "Namespace stdin JSON under this prefix")] prefix: Option<String>,
        #[param(help = "Return all context entries without filtering")] all: bool,
        #[param(help = "Context directory name inside .normalize/ (default: context)")]
        from: Option<String>,
        #[param(help = "Show source file paths only, not content")] list: bool,
    ) -> Result<ContextKindReport, String> {
        let root_path = root
            .map(PathBuf::from)
            .map(Ok)
            .unwrap_or_else(std::env::current_dir)
            .map_err(|e| format!("Failed to get current directory: {e}"))?;

        let dir_name = from.as_deref().unwrap_or("context");

        if list {
            let files = collect_new_context_files(&root_path, dir_name);
            return Ok(ContextKindReport::List(ContextListReport::new(files)));
        }

        // Build caller context from --match pairs and optionally --stdin.
        let mut caller_ctx: CallerContext = parse_match_pairs(&r#match)?;
        if stdin {
            let stdin_ctx = read_stdin_context(prefix.as_deref())?;
            caller_ctx.extend(stdin_ctx);
        }

        let raw = resolve_context(&root_path, dir_name, &caller_ctx, all);
        let blocks: Vec<ContextBlock> = raw
            .into_iter()
            .map(|(source, metadata, body)| ContextBlock {
                source,
                metadata: yaml_to_json(metadata),
                body,
            })
            .collect();

        Ok(ContextKindReport::Full(ContextReport::new(blocks)))
    }

    /// Initialize normalize in current directory
    ///
    /// Examples:
    ///   normalize init                         # create .normalize/ config directory
    ///   normalize init --setup                 # interactive rule setup wizard
    #[cli(display_with = "display_output")]
    pub async fn init(
        &self,
        #[param(help = "Index the codebase after initialization")] index: bool,
        #[param(help = "Run interactive rule setup wizard after initialization")] setup: bool,
        #[param(help = "Preview changes without writing")] dry_run: bool,
    ) -> Result<InitReport, String> {
        use std::fs;

        let root = std::env::current_dir()
            .map_err(|e| format!("Failed to get current directory: {}", e))?;
        let mut changes = Vec::new();

        // 1. Create .normalize directory if needed
        let normalize_dir = root.join(".normalize");
        if !normalize_dir.exists() {
            if !dry_run {
                fs::create_dir_all(&normalize_dir)
                    .map_err(|e| format!("Failed to create .normalize directory: {}", e))?;
            }
            changes.push("Created .normalize/".to_string());
        }

        // 2. Detect task-list files for alias config
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
            if !dry_run {
                fs::write(&config_path, default_config)
                    .map_err(|e| format!("Failed to create config.toml: {}", e))?;
            }
            changes.push("Created .normalize/config.toml".to_string());
            for f in &todo_files {
                changes.push(format!("Detected TODO file: {}", f));
            }
        }

        // 4. Update .gitignore if needed
        if !dry_run {
            let gitignore_path = root.join(".gitignore");
            let gitignore_changes = commands::init::update_gitignore(&gitignore_path);
            changes.extend(gitignore_changes);
        } else {
            // In dry-run mode, detect what would change without writing
            let gitignore_path = root.join(".gitignore");
            let gitignore_changes = commands::init::preview_gitignore_changes(&gitignore_path);
            changes.extend(gitignore_changes);
        }

        if !dry_run {
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
                commands::init::run_setup_wizard(&root);
            }

            // 8. Suggest enabling semantic search (CTA)
            println!(
                "\nSemantic search available. Enable with `embeddings.enabled = true` in .normalize/config.toml"
            );
        } else {
            if index {
                changes.push("Would index codebase".to_string());
            }
            if setup {
                changes.push("Would run interactive rule setup wizard".to_string());
            }
        }

        Ok(InitReport {
            message: if changes.is_empty() {
                "Already initialized.".to_string()
            } else if dry_run {
                format!("{} changes would be made", changes.len())
            } else {
                "Initialization complete.".to_string()
            },
            changes,
            dry_run,
        })
    }

    /// Check for and install updates
    ///
    /// Examples:
    ///   normalize update                       # check for and install updates
    #[cli(display_with = "display_output")]
    pub fn update(
        &self,
        #[param(short = 'c', help = "Check for updates without installing")] check: bool,
    ) -> Result<UpdateReport, String> {
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
            return Ok(UpdateReport {
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

        Ok(UpdateReport {
            current_version: CURRENT_VERSION.to_string(),
            latest_version,
            update_available: true,
            message: Some(
                "Updated successfully! Restart normalize to use the new version.".to_string(),
            ),
        })
    }

    /// Translate code between programming languages
    ///
    /// Examples:
    ///   normalize translate src/main.py --to rust    # translate Python to Rust
    ///   normalize translate lib.rs --to typescript    # translate Rust to TypeScript
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
    ) -> Result<TranslateReport, String> {
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
            Ok(TranslateReport {
                code,
                source_language: source_lang.to_string(),
                target_language: target_lang.to_string(),
                input_path: input,
                output_path: Some(path.clone()),
            })
        } else {
            Ok(TranslateReport {
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

    /// Workflow guides with examples
    pub fn guide(&self) -> &guide::GuideService {
        &self.guide
    }

    /// Generate code from API spec
    pub fn generate(&self) -> &generate::GenerateService {
        &self.generate
    }

    /// Manage the structural index (symbols, imports, calls)
    pub fn structure(&self) -> &facts::FactsService {
        &self.structure
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

    /// Rank code by metrics (complexity, size, coupling, duplicates, and more)
    pub fn rank(&self) -> &rank::RankService {
        &self.rank
    }

    /// Track health metrics (complexity, length, test ratio, density) over git history
    pub fn trend(&self) -> &trend::TrendService {
        &self.trend
    }

    /// Track diff-based budgets (limits on how much things can change)
    pub fn budget(&self) -> &normalize_budget::service::BudgetService {
        &self.budget
    }

    /// Track metric regressions with a ratchet baseline
    pub fn ratchet(&self) -> &normalize_ratchet::service::RatchetService {
        &self.ratchet
    }

    /// Manage and run syntax/fact rules
    pub fn rules(&self) -> &normalize_rules::RulesService {
        &self.rules
    }

    /// Inspect and validate config files using JSON Schema
    pub fn config(&self) -> &config::ConfigService {
        &self.config
    }

    /// Start a normalize server (MCP, HTTP, LSP)
    pub fn serve(&self) -> &serve::ServeService {
        &self.serve
    }

    /// Run all configured quality checks and exit non-zero if any errors are found.
    ///
    /// Runs the syntax rules engine (tree-sitter queries), native rules engine (stale-summary,
    /// ratchet, budget), and fact rules engine in sequence. Returns a `CiReport` with grouped
    /// findings. Use `--strict` to treat warnings as errors; `--sarif` for GitHub Actions output.
    ///
    /// If the structural index has not been built, fact rules are skipped with a warning
    /// diagnostic rather than erroring out. Run `normalize structure rebuild` to build the index.
    ///
    /// Examples:
    ///   normalize ci                           # run all engines, exit 1 on errors
    ///   normalize ci --path src/               # scope run to a subdirectory
    ///   normalize ci --no-native               # skip native checks (stale-summary, ratchet, budget)
    ///   normalize ci --strict                  # treat warnings as errors
    ///   normalize ci --sarif                   # SARIF output for GitHub Actions annotations
    ///   normalize ci --json                    # structured JSON output
    #[cli(display_with = "display_output")]
    #[allow(clippy::too_many_arguments)]
    pub async fn ci(
        &self,
        #[param(help = "Skip syntax rules engine")] no_syntax: bool,
        #[param(
            help = "Skip native rules engine (stale-summary, stale-docs, check-examples, check-refs, ratchet, budget)"
        )]
        no_native: bool,
        #[param(help = "Skip fact rules engine")] no_fact: bool,
        #[param(help = "Treat warnings as errors (exit 1 on any warning)")] strict: bool,
        #[param(help = "Output in SARIF format for GitHub Actions")] sarif: bool,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
        #[param(short = 'p', help = "Scope run to this directory (relative to root)")] path: Option<
            String,
        >,
        #[param(help = "Maximum number of issues to show in detail (default: 50)")] limit: Option<
            usize,
        >,
        pretty: bool,
        compact: bool,
    ) -> Result<commands::ci::CiReport, String> {
        use normalize_output::diagnostics::{DiagnosticsReport, Issue, Severity};
        use normalize_rules::{
            RuleKind, apply_native_rules_config, load_rules_config, run_rules_report,
        };
        use std::time::Instant;

        let effective_root = root
            .as_deref()
            .map(std::path::PathBuf::from)
            .map(Ok)
            .unwrap_or_else(std::env::current_dir)
            .map_err(|e| format!("Failed to get current directory: {e}"))?;
        self.resolve_format(pretty, compact, &effective_root);

        // Target scope: --path scopes runs to a subdirectory (defaults to project root).
        let target_root = path
            .as_deref()
            .map(|p| effective_root.join(p))
            .unwrap_or_else(|| effective_root.clone());

        let _limit = limit.unwrap_or(50).min(10_000);

        let start = Instant::now();
        let mut merged = DiagnosticsReport::new();
        let mut engines_run: Vec<String> = Vec::new();

        // Syntax engine
        if !no_syntax {
            let root_clone = effective_root.clone();
            let target_clone = target_root.clone();
            let config = load_rules_config(&root_clone);
            let report = tokio::task::spawn_blocking(move || {
                run_rules_report(
                    &target_clone,
                    &root_clone,
                    None,
                    None,
                    &RuleKind::Syntax,
                    &[],
                    &config,
                    None,
                    &normalize_rules_config::PathFilter::default(),
                )
            })
            .await
            .map_err(|e| format!("Task error (syntax): {e}"))?;
            merged.merge(report);
            engines_run.push("syntax".into());
        }

        // Native engine (stale-summary, check-refs, check-examples, ratchet, budget)
        if !no_native {
            let native_root = effective_root.clone();
            let native_config = load_rules_config(&native_root);
            let threshold = 10;

            #[derive(serde::Deserialize, Default)]
            struct SummaryRuleConfig {
                #[serde(
                    default,
                    deserialize_with = "normalize_rules_config::deserialize_one_or_many"
                )]
                filenames: Vec<String>,
                #[serde(
                    default,
                    deserialize_with = "normalize_rules_config::deserialize_one_or_many"
                )]
                paths: Vec<String>,
            }

            let stale_summary_cfg: SummaryRuleConfig = native_config
                .rules
                .rules
                .get("stale-summary")
                .map(|r| r.rule_config())
                .unwrap_or_default();
            let stale_summary_filenames = stale_summary_cfg.filenames;
            let stale_summary_paths = stale_summary_cfg.paths;

            let (summary_res, stale_res, examples_res, refs_res, ratchet_res, budget_res) = tokio::join!(
                tokio::task::spawn_blocking({
                    let root = native_root.clone();
                    move || {
                        normalize_native_rules::build_stale_summary_report(
                            &root,
                            threshold,
                            &stale_summary_filenames,
                            &stale_summary_paths,
                        )
                    }
                }),
                tokio::task::spawn_blocking({
                    let root = native_root.clone();
                    move || normalize_native_rules::build_stale_docs_report(&root)
                }),
                tokio::task::spawn_blocking({
                    let root = native_root.clone();
                    move || normalize_native_rules::build_check_examples_report(&root)
                }),
                normalize_native_rules::build_check_refs_report(&native_root),
                tokio::task::spawn_blocking({
                    let root = native_root.clone();
                    move || normalize_native_rules::build_ratchet_report(&root)
                }),
                tokio::task::spawn_blocking({
                    let root = native_root.clone();
                    move || normalize_native_rules::build_budget_report(&root)
                }),
            );
            let mut native_report = DiagnosticsReport::new();
            if let Ok(r) = summary_res {
                native_report.merge(r.into());
            }
            if let Ok(r) = stale_res {
                native_report.merge(r.into());
            }
            if let Ok(r) = examples_res {
                native_report.merge(r.into());
            }
            if let Ok(r) = refs_res {
                native_report.merge(r.into());
            }
            if let Ok(r) = ratchet_res {
                native_report.merge(r.into());
            }
            if let Ok(r) = budget_res {
                native_report.merge(r.into());
            }
            apply_native_rules_config(&mut native_report, &native_config.rules);
            native_report.sources_run.push("native".into());
            merged.merge(native_report);
            engines_run.push("native".into());
        }

        // Fact engine — requires a built index. If the index does not exist, emit a warning
        // diagnostic and skip rather than failing or auto-building (which can be slow in CI).
        if !no_fact {
            let normalize_dir = effective_root.join(".normalize");
            let index_path = normalize_dir.join("index.sqlite");
            if !index_path.exists() {
                tracing::warn!(
                    "fact rules skipped: index not built at {:?}; run `normalize structure rebuild`",
                    index_path
                );
                merged.issues.push(Issue {
                    rule_id: "ci/fact-rules-skipped".into(),
                    file: ".normalize/index.sqlite".into(),
                    line: None,
                    column: None,
                    end_line: None,
                    end_column: None,
                    message:
                        "fact rules skipped: index not built — run `normalize structure rebuild`"
                            .into(),
                    severity: Severity::Warning,
                    source: "ci".into(),
                    related: Vec::new(),
                    suggestion: Some("run `normalize structure rebuild` to build the index".into()),
                });
            } else {
                let fact_root = effective_root.clone();
                let target_clone = target_root.clone();
                let config = load_rules_config(&fact_root);
                let report = tokio::task::spawn_blocking(move || {
                    run_rules_report(
                        &target_clone,
                        &fact_root,
                        None,
                        None,
                        &RuleKind::Fact,
                        &[],
                        &config,
                        None,
                        &normalize_rules_config::PathFilter::default(),
                    )
                })
                .await
                .map_err(|e| format!("Task error (fact): {e}"))?;
                merged.merge(report);
                engines_run.push("fact".into());
            }
        }

        merged.sort();
        let duration_ms = start.elapsed().as_millis() as u64;
        let report = commands::ci::CiReport::new(merged, engines_run, duration_ms);

        // Determine exit condition
        let error_count = report.error_count();
        let warning_count = report.warning_count();
        let has_errors = error_count > 0;
        let has_strict_failures = strict && warning_count > 0;

        if has_errors || has_strict_failures {
            let detail = if sarif {
                report.diagnostics.format_sarif()
            } else {
                self.display_output(&report)
            };
            let msg = if has_strict_failures && !has_errors {
                format!("{detail}\n{warning_count} warning(s) found (--strict mode)")
            } else {
                format!("{detail}\n{error_count} error(s) found")
            };
            return Err(msg);
        }

        Ok(report)
    }
}

impl OutputFormatter for InitReport {
    fn format_text(&self) -> String {
        use std::fmt::Write as _;
        let mut out = String::new();
        if self.dry_run {
            let _ = writeln!(out, "[dry-run] Would initialize normalize:");
            if self.changes.is_empty() {
                let _ = write!(out, "  (no changes needed)");
            } else {
                for change in &self.changes {
                    let _ = writeln!(out, "  {}", change);
                }
            }
        } else {
            let _ = write!(out, "{}", self.message);
        }
        out
    }
}
