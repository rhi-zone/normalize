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
pub mod context;
pub mod daemon;
pub mod docs;
pub mod edit;
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
pub mod syntax;
pub mod tools;
pub mod trend;
pub mod view;

use crate::commands;
use crate::commands::aliases::{AliasesReport, detect_project_languages};
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
    /// Raw `--pretty`/`--compact` flag values delivered by `CliGlobals`, resolved
    /// per-command against the target root (TTY/config) into `pretty`.
    pretty_raw: Cell<bool>,
    compact_raw: Cell<bool>,
    analyze: analyze::AnalyzeService,
    architecture: normalize_architecture::ArchitectureService,
    config: config::ConfigService,
    context: context::ContextService,
    daemon: daemon::DaemonService,
    edit: edit::EditService,
    structure: normalize_facts::service::FactsCliService,
    filter: normalize_filter::service::FilterCliService,
    grammars: grammars::GrammarService,
    guide: guide::GuideService,
    generate: generate::GenerateService,
    graph: normalize_graph::GraphService,
    package: package::PackageService,
    rank: rank::RankService,
    similarity: normalize_code_similarity::SimilarityService,
    budget: normalize_budget::service::BudgetService,
    cfg: normalize_cfg::service::CfgService,
    kg: normalize_knowledge_graph::service::KgCliService,
    ratchet: normalize_ratchet::service::RatchetService,
    rules: normalize_rules::RulesService,
    serve: serve::ServeService,
    syntax: syntax::SyntaxService,
    sessions: normalize_sessions::service::SessionsService,
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

impl server_less::CliGlobals for NormalizeService {
    fn set_global_flag(&self, name: &str, value: bool) {
        match name {
            "pretty" => self.pretty_raw.set(value),
            "compact" => self.compact_raw.set(value),
            _ => {}
        }
    }
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
            architecture: normalize_architecture::ArchitectureService::new(&pretty),
            config: config::ConfigService::new(&pretty),
            context: context::ContextService::new(&pretty),
            daemon: daemon::DaemonService,
            edit: edit::EditService {
                history: history::HistoryService,
            },
            structure: normalize_facts::service::FactsCliService::new(),
            filter: normalize_filter::service::FilterCliService::new(),
            grammars: grammars::GrammarService::new(&pretty),
            guide: guide::GuideService,
            generate: generate::GenerateService,
            graph: normalize_graph::GraphService::new(&pretty),
            package: package::PackageService::new(),
            rank: rank::RankService::new(&pretty),
            similarity: normalize_code_similarity::SimilarityService::new(&pretty),
            budget: normalize_budget::service::BudgetService::new(),
            cfg: normalize_cfg::service::CfgService::new(),
            kg: normalize_knowledge_graph::service::KgCliService::new(),
            ratchet: normalize_ratchet::service::RatchetService::new(),
            rules: normalize_rules::RulesService::new(&pretty),
            serve: serve::ServeService,
            syntax: syntax::SyntaxService::new(),
            sessions: normalize_sessions::service::SessionsService::new(&pretty),
            tools: tools::ToolsService::new(),
            trend: trend::TrendService::new(&pretty),
            view: view::ViewService::new(&pretty),
            pretty,
            pretty_raw: Cell::new(false),
            compact_raw: Cell::new(false),
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

    /// Resolve pretty/compact state from the sink-delivered raw global flags and
    /// config (TTY auto-detection), resolved against `root`, into `self.pretty`.
    fn resolve_format(&self, root: &std::path::Path) {
        self.pretty.set(resolve_pretty(
            root,
            self.pretty_raw.get(),
            self.compact_raw.get(),
        ));
    }

    /// Generic display bridge that respects pretty/compact state.
    fn display_output<T: OutputFormatter>(&self, value: &T) -> String {
        if self.pretty.get() {
            value.format_pretty()
        } else {
            value.format_text()
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
#[server(groups(
    core = "Core",
    analysis = "Analysis",
    utilities = "Utilities",
    infrastructure = "Infrastructure",
))]
impl NormalizeService {
    /// Browse code structure and symbol relationships. Use to read files, explore types, or trace dependencies.
    #[server(group = "core")]
    pub fn view(&self) -> &view::ViewService {
        &self.view
    }

    /// Find code by text pattern. Use when you know what the code looks like but not where it is.
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
    #[server(group = "core")]
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
    ) -> Result<GrepReport, String> {
        // `path` positional takes precedence over `--root` flag.
        let root_path = path
            .or(root)
            .map(PathBuf::from)
            .map(Ok)
            .unwrap_or_else(std::env::current_dir)
            .map_err(|e| format!("Failed to get current directory: {e}"))?;

        self.resolve_format(&root_path);

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

    /// Transitional hidden alias — `aliases` now lives at `filter aliases`.
    ///
    /// Kept for one release so existing scripts keep working; removed at 1.0.
    #[server(group = "utilities")]
    #[cli(hidden, display_with = "display_output")]
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

    /// Inject project context into LLM prompts. Use to provide per-project instructions to agents.
    ///
    /// Resolves Markdown files from .normalize/context/ directories, walked bottom-up
    /// (project → parent → ~/.normalize/context/). Each .md file may have YAML frontmatter;
    /// blocks whose frontmatter matches the caller context are included. Bare files always match.
    ///
    /// FRONTMATTER FORMAT (.normalize/context/hints.md):
    ///   --- / claudecode: / hook: UserPromptSubmit / scope: / language: rust / ---
    ///   Body text included when caller context matches.
    ///   Multiple blocks per file separated by ---. Frontmatter is arbitrary nested YAML.
    ///
    /// --match SYNTAX (dot-path KEY=VALUE, repeatable, ANDed):
    ///   --match hook=UserPromptSubmit            (simple key)
    ///   --match claudecode.hook=UserPromptSubmit (nested dot-path)
    ///
    /// --stdin / --prefix (pipe JSON as caller context):
    ///   echo '{"hook":"UserPromptSubmit"}' | normalize context --stdin --prefix claudecode
    ///
    /// --file PREFIX=PATH (load .json/.toml/.yaml into context):
    ///   normalize context --file cfg=config.toml
    ///
    /// EXAMPLES:
    ///   normalize context                                             (all matching, no filter)
    ///   normalize context --match claudecode.hook=UserPromptSubmit   (Claude Code hook shim)
    ///   cat | normalize context --stdin --prefix claudecode          (pipe stdin as context)
    ///   normalize context --all --list                               (list all source files)
    ///   normalize context migrate --apply                            (migrate .context.md files)
    #[server(group = "utilities")]
    pub fn context(&self) -> &context::ContextService {
        &self.context
    }

    /// Set up normalize in a new project. Run once after cloning to create .normalize/ config.
    ///
    /// Examples:
    ///   normalize init                         # create .normalize/ config directory
    ///   normalize init --setup                 # interactive rule setup wizard
    #[server(group = "core")]
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

            // Seed a discoverable `[walk] exclude` section: the daemon baseline
            // (`.git/`, `.normalize/`) plus any auto-detected scratch dirs
            // (e.g. `.claude/worktrees/`) present under the target root.
            let walk = commands::init::build_walk_section(&root);

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
{}{}"#,
                walk.toml, aliases_section
            );
            if !dry_run {
                fs::write(&config_path, default_config)
                    .map_err(|e| format!("Failed to create config.toml: {}", e))?;
            }
            changes.push("Created .normalize/config.toml".to_string());
            changes.push(walk.summary);
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
            // 6. Optionally index
            if index {
                tracing::info!("indexing codebase...");
                let mut idx = crate::index::open(&root)
                    .await
                    .map_err(|e| format!("Failed to open index: {}", e))?;
                let count = idx
                    .refresh()
                    .await
                    .map_err(|e| format!("Failed to index: {}", e))?;
                tracing::info!("indexed {} files", count);
            }

            // 7. Optionally run setup wizard
            if setup {
                commands::init::run_setup_wizard(&root);
            }
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

    /// Check for and install newer versions of normalize.
    ///
    /// Examples:
    ///   normalize update                       # check for and install updates
    #[server(group = "infrastructure")]
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
        tracing::info!("downloading update...");

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

        tracing::info!("  downloading {}...", asset_name);
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
            tracing::info!("  verifying checksum...");
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
        tracing::info!("  extracting...");
        let binary_data = if asset_name.ends_with(".tar.gz") {
            commands::update::extract_tar_gz(&archive_data)
        } else if asset_name.ends_with(".zip") {
            commands::update::extract_zip(&archive_data)
        } else {
            Err(format!("Unknown archive format: {}", asset_name))
        }?;

        // Replace current binary
        tracing::info!("  installing...");
        commands::update::self_replace(&binary_data)?;

        // Invalidate the grammar stamp so the next invocation re-downloads
        // grammars matching the new binary version.  The stamp path mirrors
        // what `commands::grammars::user_grammars_dir()` returns.
        if let Some(grammar_dir) = dirs::config_dir().map(|c| c.join("normalize/grammars")) {
            let stamp = grammar_dir.join(".installed-version");
            let _ = std::fs::remove_file(&stamp);
        }

        Ok(UpdateReport {
            current_version: CURRENT_VERSION.to_string(),
            latest_version,
            update_available: true,
            message: Some(
                "Updated successfully! Restart normalize to use the new version.".to_string(),
            ),
        })
    }

    /// Convert code between programming languages. Use for porting or understanding unfamiliar syntax.
    ///
    /// Examples:
    ///   normalize translate src/main.py --to rust    # translate Python to Rust
    ///   normalize translate lib.rs --to typescript    # translate Rust to TypeScript
    #[server(group = "utilities")]
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
            tracing::info!("translated {} -> {} ({})", input, path, target_lang);
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

    /// Control the background daemon that keeps the index fresh automatically.
    #[server(group = "infrastructure")]
    pub fn daemon(&self) -> &daemon::DaemonService {
        &self.daemon
    }

    /// Install and list tree-sitter grammars. Run after install or when parsing fails for a language.
    #[server(group = "infrastructure")]
    pub fn grammars(&self) -> &grammars::GrammarService {
        &self.grammars
    }

    /// Step-by-step workflow guides. Use when learning normalize or onboarding a new codebase.
    #[server(group = "utilities")]
    pub fn guide(&self) -> &guide::GuideService {
        &self.guide
    }

    /// Generate code from an API spec. Use to scaffold clients or types from OpenAPI definitions.
    #[server(group = "utilities")]
    pub fn generate(&self) -> &generate::GenerateService {
        &self.generate
    }

    /// Build and query the code index. Run `structure rebuild` after cloning or when cross-file commands return stale results.
    #[server(group = "core")]
    pub fn structure(&self) -> &normalize_facts::service::FactsCliService {
        &self.structure
    }

    /// Filter files by glob patterns and inspect --exclude/--only aliases.
    ///
    /// Examples:
    ///   normalize filter aliases                       # list all filter aliases
    ///   normalize filter matches src/main.rs --only "*.rs"
    #[server(group = "utilities")]
    pub fn filter(&self) -> &normalize_filter::service::FilterCliService {
        &self.filter
    }

    /// Inspect parsed syntax trees and test queries. Use to debug grammars or develop tree-sitter patterns.
    #[server(group = "infrastructure")]
    pub fn syntax(&self) -> &syntax::SyntaxService {
        &self.syntax
    }

    /// Query package metadata and dependencies. Use to check versions, find outdated deps, or view dep trees.
    #[server(group = "utilities")]
    pub fn package(&self) -> &package::PackageService {
        &self.package
    }

    /// Fetch upstream symbol documentation into LLM context.
    ///
    /// Retrieves current documentation from docs.rs for Rust symbols and outputs a
    /// Markdown block ready to paste into an LLM prompt. Results are cached in the
    /// knowledge graph so repeat lookups are instant. Patches training-cutoff blind spots.
    ///
    /// Accepts Rust path syntax. Append `@version` to pin a specific version.
    /// If invoked inside a Cargo project, the lockfile version is used automatically.
    ///
    /// Examples:
    ///   normalize docs serde::Serialize
    ///   normalize docs tokio::sync::Mutex
    ///   normalize docs serde                           # crate-level docs
    ///   normalize docs serde::Serialize@1.0.193       # pin a specific version
    ///   normalize docs serde::Serialize --no-cache    # bypass local cache
    #[server(group = "utilities")]
    #[cli(display_with = "display_output")]
    pub fn docs(
        &self,
        #[param(
            positional,
            help = "Symbol path, e.g. serde::Serialize or serde::Serialize@1.0.193"
        )]
        symbol: String,
        #[param(
            short = 'r',
            help = "Root directory for lockfile lookup (defaults to current directory)"
        )]
        root: Option<String>,
        #[param(
            short = 'e',
            help = "Ecosystem to query (e.g. cargo); auto-detected from the project when omitted"
        )]
        ecosystem: Option<String>,
        #[param(help = "Bypass the local knowledge-graph cache and always fetch from the network")]
        no_cache: bool,
    ) -> Result<docs::DocsReport, String> {
        let root_path = root
            .map(PathBuf::from)
            .map(Ok)
            .unwrap_or_else(std::env::current_dir)
            .map_err(|e| format!("Failed to get current directory: {e}"))?;

        self.resolve_format(&root_path);

        docs::fetch_docs(&symbol, root_path, no_cache, ecosystem)
    }

    /// Review AI agent session logs. Use to check cost, duration, and tool usage across coding sessions.
    #[server(group = "utilities")]
    pub fn sessions(&self) -> &normalize_sessions::service::SessionsService {
        &self.sessions
    }

    /// Copy a project and its session metadata to a destination for portability.
    ///
    /// Copies the project directory (excluding `target/`, `node_modules/`, `.git/objects/`,
    /// `.normalize/findings-cache.sqlite`, `.fastembed_cache/`) and any associated AI agent
    /// session metadata to `<dest>`. After copying, rewrites absolute paths in the index DB
    /// so the copy works from its new location.
    ///
    /// On subsequent syncs, skips files whose content hash matches the manifest stored at
    /// `<dest>/.normalize/sync-manifest.json`. Use `--force` to bypass the manifest.
    ///
    /// Session metadata is discovered across all registered AI agent formats (Claude Code,
    /// OpenAI Codex, Gemini CLI, Normalize Agent) via the `normalize_chat_sessions` format
    /// registry.
    ///
    /// Examples:
    ///   normalize sync /backup/myproject               # copy project to new location
    ///   normalize sync /backup/myproject --dry-run     # preview what would be copied
    ///   normalize sync /backup/myproject --verbose     # show each file as it's copied
    ///   normalize sync /backup/myproject --force       # force full re-sync (ignore manifest)
    ///   normalize sync /backup --all                   # copy all known projects
    ///   normalize sync /backup --all --active 30       # only projects active in last 30 days
    ///   normalize sync /backup --all --repo "*/rhizone/*"  # filter by path glob
    #[server(group = "utilities")]
    #[cli(display_with = "display_output")]
    #[allow(clippy::too_many_arguments)]
    pub async fn sync(
        &self,
        #[param(positional, help = "Destination directory")] dest: Option<String>,
        #[param(help = "Copy all known projects (discovered via session metadata)")] all: bool,
        #[param(
            short = 'r',
            help = "Source project root (defaults to current directory)"
        )]
        root: Option<String>,
        #[param(help = "Dry run — show what would be copied without writing anything")]
        dry_run: bool,
        #[param(short = 'v', help = "Print each file as it is copied")] verbose: bool,
        #[param(help = "Force full re-sync, ignoring the incremental manifest")] force: bool,
        #[param(
            help = "Only sync projects with activity in the last N days (--all only, default 30)"
        )]
        active: Option<u32>,
        #[param(help = "Only sync projects whose path matches this glob (--all only)")]
        repo: Option<String>,
        #[param(help = "Exclude projects whose path matches this glob (--all only)")]
        exclude: Option<String>,
    ) -> Result<commands::sync::SyncReport, String> {
        use commands::sync::{
            SyncFileItem, SyncManifest, SyncReport, common_prefix, copy_tree_incremental,
            list_all_known_project_roots, rewrite_index_paths,
        };
        use normalize_chat_sessions::project_metadata_roots;
        use std::time::{Duration, SystemTime};

        let dest_str = dest.ok_or_else(|| {
            "Destination required. Usage: normalize sync <dest> [--all]".to_string()
        })?;
        let dest_root = PathBuf::from(&dest_str);

        let root_path = root
            .map(PathBuf::from)
            .map(Ok)
            .unwrap_or_else(std::env::current_dir)
            .map_err(|e| format!("Failed to get current directory: {e}"))?;

        self.resolve_format(&root_path);

        // Determine the list of project roots to sync.
        let project_roots: Vec<PathBuf> = if all {
            let mut roots = list_all_known_project_roots();

            // --active N: filter by last activity within N days
            if let Some(days) = active.or(Some(30)) {
                let cutoff = SystemTime::now() - Duration::from_secs(days as u64 * 86400);
                roots.retain(|p| {
                    // Check mtime of the project dir itself; also scan .normalize/
                    let normalize_dir = p.join(".normalize");
                    let check_dir = if normalize_dir.exists() {
                        &normalize_dir
                    } else {
                        p
                    };
                    std::fs::metadata(check_dir)
                        .and_then(|m| m.modified())
                        .map(|mtime| mtime >= cutoff)
                        .unwrap_or(false)
                });
            }

            // --repo glob filter
            if let Some(ref glob_pat) = repo {
                roots.retain(|p| {
                    glob::Pattern::new(glob_pat)
                        .map(|pat| pat.matches(&p.to_string_lossy()))
                        .unwrap_or(false)
                });
            }

            // --exclude glob filter
            if let Some(ref exc_pat) = exclude {
                roots.retain(|p| {
                    glob::Pattern::new(exc_pat)
                        .map(|pat| !pat.matches(&p.to_string_lossy()))
                        .unwrap_or(true)
                });
            }

            if roots.is_empty() {
                tracing::warn!(
                    "no projects found matching the given filters; syncing current directory"
                );
                vec![root_path.clone()]
            } else {
                roots
            }
        } else {
            vec![root_path.clone()]
        };

        // For multi-project sync: strip common prefix to get relative dest structure.
        let prefix = if project_roots.len() > 1 {
            common_prefix(&project_roots)
        } else {
            None
        };

        let mut total_files = 0usize;
        let mut total_unchanged = 0usize;
        let mut total_sessions = 0usize;
        let mut all_file_items: Vec<SyncFileItem> = Vec::new();
        let mut all_warnings: Vec<String> = Vec::new();
        let mut index_rewritten = false;

        for proj_root in &project_roots {
            // Compute the destination for this project.
            let proj_dest = if let Some(ref pfx) = prefix {
                let rel = proj_root.strip_prefix(pfx).unwrap_or(proj_root);
                dest_root.join(rel)
            } else {
                dest_root.clone()
            };

            // 1. Load incremental manifest from destination.
            let manifest = if force || dry_run {
                SyncManifest::default()
            } else {
                SyncManifest::load(&proj_dest)
            };

            // 2. Copy project tree (incremental).
            let (n, unchanged, items, new_entries) = copy_tree_incremental(
                proj_root,
                &proj_dest,
                dry_run,
                verbose,
                force,
                &manifest,
                &mut all_warnings,
            );
            total_files += n;
            total_unchanged += unchanged;
            all_file_items.extend(items);

            // 3. Save updated manifest.
            if !dry_run {
                let mut updated = SyncManifest::default();
                // Preserve unchanged entries + add newly computed entries
                for (k, v) in &manifest.files {
                    updated.files.insert(k.clone(), v.clone());
                }
                for (k, v) in new_entries {
                    updated.files.insert(k, v);
                }
                if let Err(e) = updated.save(&proj_dest) {
                    all_warnings.push(format!("manifest save: {}", e));
                }
            }

            // 4. Copy session metadata for all registered formats.
            for meta_root in project_metadata_roots(proj_root) {
                let format_name = meta_root
                    .parent()
                    .and_then(|p| p.file_name())
                    .and_then(|n| n.to_str())
                    .unwrap_or("sessions");
                let sessions_dest = proj_dest
                    .join(".sessions")
                    .join(format_name)
                    .join(meta_root.file_name().unwrap_or_default());
                // Session metadata is not incremental (always copy; manifests track project files)
                let (sn, _unchanged, sitems, _) = copy_tree_incremental(
                    &meta_root,
                    &sessions_dest,
                    dry_run,
                    verbose,
                    true,
                    &SyncManifest::default(),
                    &mut all_warnings,
                );
                total_sessions += sn;
                all_file_items.extend(sitems);
            }

            // 5. Rewrite index paths.
            if !dry_run {
                let dest_db = proj_dest.join(".normalize").join("index.sqlite");
                if dest_db.exists() {
                    let old_root = proj_root.to_string_lossy().into_owned();
                    let new_root = proj_dest.to_string_lossy().into_owned();
                    if let Err(e) = rewrite_index_paths(&dest_db, &old_root, &new_root).await {
                        all_warnings.push(format!("index path rewrite: {}", e));
                    } else {
                        index_rewritten = true;
                    }
                }
            }
        }

        let source = if project_roots.len() == 1 {
            project_roots[0].to_string_lossy().into_owned()
        } else {
            format!("{} projects", project_roots.len())
        };

        Ok(SyncReport {
            dest: dest_str,
            source,
            files_copied: total_files,
            files_unchanged: total_unchanged,
            sessions_copied: total_sessions,
            index_paths_rewritten: index_rewritten,
            dry_run,
            files: all_file_items,
            warnings: all_warnings,
        })
    }

    /// Run linters, formatters, and test runners. Unified interface to external ecosystem tools.
    #[server(group = "infrastructure")]
    pub fn tools(&self) -> &tools::ToolsService {
        &self.tools
    }

    /// Edit code by symbol name. Use for batch renames, signature changes, or pattern-based rewrites.
    #[server(group = "core")]
    pub fn edit(&self) -> &edit::EditService {
        &self.edit
    }

    /// Assess codebase quality. Use for health checks, finding duplicates, security scanning, and architecture analysis.
    #[server(group = "analysis")]
    pub fn analyze(&self) -> &analyze::AnalyzeService {
        &self.analyze
    }

    /// Rank files and functions by metrics. Use to find the most complex, longest, or most coupled code.
    #[server(group = "analysis")]
    pub fn rank(&self) -> &rank::RankService {
        &self.rank
    }

    /// Plot metrics over git history. Use to see if complexity, size, or test coverage is trending up or down.
    #[server(group = "analysis")]
    pub fn trend(&self) -> &trend::TrendService {
        &self.trend
    }

    /// Enforce diff budgets on PRs. Use to cap how much complexity or size can grow per change.
    #[server(group = "analysis")]
    pub fn budget(&self) -> &normalize_budget::service::BudgetService {
        &self.budget
    }

    /// Analyze architectural structure: coupling, cycles, hub modules, layering, and depth.
    ///
    /// Examples:
    ///   normalize architecture                   # coupling, hubs, layer flows
    ///   normalize architecture layering          # import-direction compliance
    ///   normalize architecture depth-map         # dependency depth + ripple risk
    #[server(group = "analysis")]
    pub fn architecture(&self) -> &normalize_architecture::ArchitectureService {
        &self.architecture
    }

    /// Analyze the dependency graph: cycles, blast radius, import paths. Requires the facts index.
    ///
    /// Examples:
    ///   normalize graph                          # module dependency graph
    ///   normalize graph --on symbols             # symbol-level graph
    ///   normalize graph dependents src/lib.rs    # what depends on this file
    ///   normalize graph import-path src/a.rs src/b.rs  # shortest import chain
    #[server(group = "analysis")]
    pub fn graph(&self) -> &normalize_graph::GraphService {
        &self.graph
    }

    /// Detect duplicate and near-duplicate code: clones, duplicate types, and repeated AST fragments.
    ///
    /// Examples:
    ///   normalize similarity                       # exact duplicate functions
    ///   normalize similarity --mode similar        # fuzzy near-duplicates (MinHash)
    ///   normalize similarity --mode clusters       # connected-component clusters
    ///   normalize similarity duplicate-types       # duplicate struct/enum/class definitions
    ///   normalize similarity fragments             # repeated AST sub-patterns
    #[server(group = "analysis")]
    pub fn similarity(&self) -> &normalize_code_similarity::SimilarityService {
        &self.similarity
    }

    /// Build and render the control flow graph for a function. Use to visualize execution paths, branches, and loops.
    #[server(group = "analysis")]
    pub fn cfg(&self) -> &normalize_cfg::service::CfgService {
        &self.cfg
    }

    /// Prevent metric regressions. Records a baseline and fails CI if metrics get worse.
    #[server(group = "analysis")]
    pub fn ratchet(&self) -> &normalize_ratchet::service::RatchetService {
        &self.ratchet
    }

    /// Configure and run lint rules. Use to enable/disable checks or see what rules are available.
    #[server(group = "core")]
    pub fn rules(&self) -> &normalize_rules::RulesService {
        &self.rules
    }

    /// Persistent knowledge graph adjacent to code. Use to create units, link concepts, and query relationships.
    ///
    /// Units are addressable Markdown documents (YAML frontmatter + body) stored in `.normalize/kg/`.
    /// Edges are directed typed relationships, stored append-only in `edges.jsonl`.
    ///
    /// Examples:
    ///   echo "Design notes." | normalize kg write --id my-design --metadata tag=design
    ///   normalize kg read my-design
    ///   normalize kg walk my-design --depth 2
    #[server(group = "core")]
    pub fn kg(&self) -> &normalize_knowledge_graph::service::KgCliService {
        &self.kg
    }

    /// Inspect and validate .normalize/config.toml. Use to debug config issues or see available options.
    #[server(group = "infrastructure")]
    pub fn config(&self) -> &config::ConfigService {
        &self.config
    }

    /// Start a normalize server. Use to expose normalize over MCP, HTTP, or LSP for editor integration.
    #[server(group = "infrastructure")]
    pub fn serve(&self) -> &serve::ServeService {
        &self.serve
    }

    /// Run all quality checks in one pass. Use in CI pipelines or before committing to catch violations.
    ///
    /// Runs the syntax rules engine (tree-sitter queries), native rules engine (check-refs,
    /// ratchet, budget), and fact rules engine in sequence. Returns a `CiReport` with grouped
    /// findings. Use `--strict` to treat warnings as errors; `--sarif` for GitHub Actions output.
    ///
    /// If the structural index has not been built, fact rules are skipped with a warning
    /// diagnostic rather than erroring out. Run `normalize structure rebuild` to build the index.
    ///
    /// Examples:
    ///   normalize ci                           # run all engines, exit 1 on errors
    ///   normalize ci --path src/               # scope run to a subdirectory
    ///   normalize ci --no-native               # skip native checks (check-refs, ratchet, budget)
    ///   normalize ci --strict                  # treat warnings as errors
    ///   normalize ci --sarif                   # SARIF output for GitHub Actions annotations
    ///   normalize ci --json                    # structured JSON output
    #[server(group = "analysis")]
    #[cli(display_with = "display_output")]
    #[allow(clippy::too_many_arguments)]
    pub async fn ci(
        &self,
        #[param(help = "Skip syntax rules engine")] no_syntax: bool,
        #[param(
            help = "Skip native rules engine (stale-docs, check-examples, check-refs, ratchet, budget)"
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
        self.resolve_format(&effective_root);

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

        // Native engine (check-refs, check-examples, ratchet, budget)
        if !no_native {
            let native_root = effective_root.clone();
            let native_config = load_rules_config(&native_root);

            let boundary_cfg: normalize_native_rules::BoundaryViolationsConfig = native_config
                .rules
                .rules
                .get("boundary-violations")
                .map(|r| r.rule_config())
                .unwrap_or_default();
            let boundaries: Vec<normalize_native_rules::Boundary> = boundary_cfg
                .boundaries
                .iter()
                .filter_map(|s| normalize_native_rules::parse_boundary(s))
                .collect();

            let (stale_res, examples_res, refs_res, ratchet_res, budget_res, boundary_res) = tokio::join!(
                tokio::task::spawn_blocking({
                    let root = native_root.clone();
                    let wc = native_config.walk.clone();
                    move || normalize_native_rules::build_stale_docs_report(&root, &wc)
                }),
                tokio::task::spawn_blocking({
                    let root = native_root.clone();
                    let wc = native_config.walk.clone();
                    move || normalize_native_rules::build_check_examples_report(&root, &wc)
                }),
                normalize_native_rules::build_check_refs_report(&native_root, &native_config.walk),
                tokio::task::spawn_blocking({
                    let root = native_root.clone();
                    move || normalize_native_rules::build_ratchet_report(&root)
                }),
                tokio::task::spawn_blocking({
                    let root = native_root.clone();
                    move || normalize_native_rules::build_budget_report(&root)
                }),
                normalize_native_rules::build_boundary_violations_report(&native_root, &boundaries),
            );
            let mut native_report = DiagnosticsReport::new();
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
            native_report.merge(boundary_res);
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
        } else if self.changes.is_empty() {
            let _ = write!(out, "{}", self.message);
        } else {
            let _ = writeln!(out, "Initialized normalize:");
            for change in &self.changes {
                let _ = writeln!(out, "  {}", change);
            }
        }
        out
    }
}
