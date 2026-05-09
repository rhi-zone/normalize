//! Context sub-service for server-less CLI.
//!
//! Hosts the default `context` command (frontmatter-filtered context resolution)
//! and the `migrate` subcommand (migration helper for old `.context.md` files).

use crate::commands::context::{
    CallerContext, ContextBlock, ContextListReport, ContextReport, collect_new_context_files,
    parse_match_pairs, read_file_context, read_stdin_context, resolve_context, yaml_to_json,
};
use crate::output::OutputFormatter;
use server_less::cli;
use std::cell::Cell;
use std::path::PathBuf;

// ---------------------------------------------------------------------------
// Report types
// ---------------------------------------------------------------------------

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

/// A single file that would be (or was) migrated.
#[derive(serde::Serialize, schemars::JsonSchema)]
pub struct ContextMigrateEntry {
    /// Source path (old `.context.md` / `CONTEXT.md`).
    pub source: String,
    /// Destination path (new `.normalize/context/<name>.md`).
    pub destination: String,
}

/// Report from `normalize context migrate`.
#[derive(serde::Serialize, schemars::JsonSchema)]
pub struct ContextMigrateReport {
    /// Whether `--apply` was passed (changes were actually written).
    pub applied: bool,
    /// Files that were (or would be) migrated.
    pub migrations: Vec<ContextMigrateEntry>,
}

impl OutputFormatter for ContextMigrateReport {
    fn format_text(&self) -> String {
        if self.migrations.is_empty() {
            return "No .context.md files found.".to_string();
        }
        let mut lines = Vec::new();
        let verb = if self.applied {
            "Migrated"
        } else {
            "Would migrate (use --apply to perform)"
        };
        lines.push(format!("{} {} file(s):", verb, self.migrations.len()));
        for entry in &self.migrations {
            lines.push(format!("  {} -> {}", entry.source, entry.destination));
        }
        lines.join("\n")
    }
}

// ---------------------------------------------------------------------------
// Daemon-backed query (v2)
// ---------------------------------------------------------------------------

/// Try to query context from the daemon's in-memory index.
///
/// Returns `Some(blocks)` on success, `None` if the daemon is not running,
/// the root is not watched, or the response cannot be decoded.
fn try_daemon_context(
    root: &std::path::Path,
    caller_ctx: &CallerContext,
    all: bool,
) -> Option<Vec<ContextBlock>> {
    let client = crate::daemon::DaemonClient::new();
    if !client.is_available() {
        return None;
    }

    let match_keys: Vec<(String, String)> = caller_ctx
        .iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();

    let response = client.query_context(root, match_keys, all, None).ok()?;

    if !response.ok {
        return None;
    }

    let arr = response.data?.as_array()?.clone();
    let blocks = arr
        .into_iter()
        .filter_map(|item| {
            let source = item.get("source")?.as_str().map(std::path::PathBuf::from)?;
            let metadata = item
                .get("metadata")
                .cloned()
                .unwrap_or(serde_json::Value::Null);
            let body = item.get("body")?.as_str()?.to_string();
            Some(ContextBlock {
                source,
                metadata,
                body,
            })
        })
        .collect();

    Some(blocks)
}

/// Filesystem fallback (v1): walk `.normalize/context/` directories and parse files.
fn resolve_context_blocks(
    root: &std::path::Path,
    dir_name: &str,
    caller_ctx: &CallerContext,
    all: bool,
) -> Vec<ContextBlock> {
    resolve_context(root, dir_name, caller_ctx, all)
        .into_iter()
        .map(|(source, metadata, body)| ContextBlock {
            source,
            metadata: yaml_to_json(metadata),
            body,
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Service
// ---------------------------------------------------------------------------

/// Context sub-service: frontmatter-filtered context resolution and migration.
pub struct ContextService {
    pretty: Cell<bool>,
}

impl ContextService {
    pub fn new(pretty: &Cell<bool>) -> Self {
        Self {
            pretty: Cell::new(pretty.get()),
        }
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
}

#[cli(
    name = "context",
    description = "Inject project context into LLM prompts.\n\
        \n\
        Resolves Markdown files from .normalize/context/ directories, walked bottom-up\n\
        (project → parent → ~/.normalize/context/). Each .md file may have YAML frontmatter;\n\
        blocks whose frontmatter matches the caller context are included. Bare files (no\n\
        frontmatter) always match.\n\
        \n\
        FRONTMATTER FORMAT\n\
          ---\n\
          claudecode:\n\
            hook: UserPromptSubmit\n\
          scope:\n\
            language: rust\n\
          ---\n\
          Body text included when caller context matches.\n\
        \n\
          Multiple blocks per file are separated by ---. Frontmatter is arbitrary nested YAML.\n\
        \n\
        --match SYNTAX\n\
          Dot-path KEY=VALUE pairs matched against frontmatter.\n\
          Simple:  --match hook=UserPromptSubmit\n\
          Nested:  --match claudecode.hook=UserPromptSubmit\n\
          Multiple --match flags are ANDed together.\n\
        \n\
        --stdin / --prefix\n\
          Read caller context as JSON from stdin. --prefix namespaces it.\n\
          echo '{\"hook\":\"UserPromptSubmit\"}' | normalize context --stdin --prefix claudecode\n\
        \n\
        --file PREFIX=PATH\n\
          Load a structured file (.json/.toml/.yaml/.yml) into caller context under PREFIX.\n\
          normalize context --file cfg=config.toml\n\
        \n\
        EXAMPLES\n\
          normalize context                                              # all matching (no filter)\n\
          normalize context --match claudecode.hook=UserPromptSubmit    # Claude Code hook shim\n\
          cat | normalize context --stdin --prefix claudecode           # pipe stdin as context\n\
          normalize context --all --list                                # list all source files\n\
          normalize context migrate --apply                             # migrate old .context.md",
    global = [
        pretty = "Human-friendly output with colors and formatting",
        compact = "Compact output without colors (overrides TTY detection)",
    ]
)]
impl ContextService {
    /// Inject project context into LLM prompts. Use to provide per-project instructions to agents.
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
    ///   normalize context --file cfg=config.toml                  # load file under prefix
    ///   normalize context migrate                                  # migrate old .context.md files
    ///   normalize context migrate --apply                         # apply migration
    #[cli(default, display_with = "display_context")]
    #[allow(clippy::too_many_arguments)]
    pub async fn context(
        &self,
        #[param(help = "Root directory for hierarchy walk (default: cwd)")] root: Option<String>,
        #[param(help = "Match context against KEY=VALUE pair (repeatable)")] r#match: Vec<String>,
        #[param(help = "Read context JSON from stdin")] stdin: bool,
        #[param(help = "Namespace stdin JSON under this prefix")] prefix: Option<String>,
        #[param(help = "Return all context entries without filtering")] all: bool,
        #[param(help = "Context directory name inside .normalize/ (default: context)")]
        from: Option<String>,
        #[param(help = "Show source file paths only, not content")] list: bool,
        #[param(
            help = "Load a structured file into caller context as PREFIX=PATH (repeatable; supports .json, .toml, .yaml/.yml)"
        )]
        file: Vec<String>,
        pretty: bool,
        compact: bool,
    ) -> Result<ContextKindReport, String> {
        let root_path = root
            .map(PathBuf::from)
            .map(Ok)
            .unwrap_or_else(std::env::current_dir)
            .map_err(|e| format!("Failed to get current directory: {e}"))?;

        self.resolve_format(pretty, compact, &root_path);

        let dir_name = from.as_deref().unwrap_or("context");

        if list {
            let files = collect_new_context_files(&root_path, dir_name);
            return Ok(ContextKindReport::List(ContextListReport::new(files)));
        }

        // Build caller context from --match pairs, optionally --stdin, and --file entries.
        let mut caller_ctx: CallerContext = parse_match_pairs(&r#match)?;
        if stdin {
            let stdin_ctx = read_stdin_context(prefix.as_deref())?;
            caller_ctx.extend(stdin_ctx);
        }
        for entry in &file {
            if let Some((pfx, path)) = entry.split_once('=') {
                let file_ctx = read_file_context(pfx, path)?;
                caller_ctx.extend(file_ctx);
            } else {
                return Err(format!(
                    "--file argument must be PREFIX=PATH, got: {entry:?}"
                ));
            }
        }

        // v2: Try daemon first for near-zero latency. Fall back to filesystem scan (v1)
        // if the daemon is unavailable, the root is not watched, or dir_name is non-default.
        let blocks = if dir_name == "context" {
            try_daemon_context(&root_path, &caller_ctx, all)
                .unwrap_or_else(|| resolve_context_blocks(&root_path, dir_name, &caller_ctx, all))
        } else {
            resolve_context_blocks(&root_path, dir_name, &caller_ctx, all)
        };

        Ok(ContextKindReport::Full(ContextReport::new(blocks)))
    }

    /// Migrate old `.context.md` / `CONTEXT.md` files to the new `.normalize/context/` system.
    ///
    /// Walks the directory tree looking for legacy context files and shows what would
    /// be moved. Each file is renamed to `.normalize/context/<dirname>.md` relative to
    /// its directory. Migrated files are bare Markdown — no frontmatter needed, since
    /// bare files always match.
    ///
    /// Without `--apply`, prints a dry-run preview. With `--apply`, performs the migration:
    /// creates the target directory, writes the new file, and removes the old one.
    ///
    /// Examples:
    ///   normalize context migrate            # preview what would be migrated
    ///   normalize context migrate --apply    # perform the migration
    #[cli(display_with = "display_output")]
    pub fn migrate(
        &self,
        #[param(help = "Root directory to search (default: cwd)")] root: Option<String>,
        #[param(help = "Perform the migration (default: dry-run preview)")] apply: bool,
        pretty: bool,
        compact: bool,
    ) -> Result<ContextMigrateReport, String> {
        let root_path = root
            .map(PathBuf::from)
            .map(Ok)
            .unwrap_or_else(std::env::current_dir)
            .map_err(|e| format!("Failed to get current directory: {e}"))?;

        self.resolve_format(pretty, compact, &root_path);

        let legacy_names = ["CONTEXT.md", ".context.md"];
        let mut migrations = Vec::new();

        for entry in walkdir::WalkDir::new(&root_path)
            .follow_links(false)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            if !entry.file_type().is_file() {
                continue;
            }
            let path = entry.path();
            let Some(file_name) = path.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            if !legacy_names.contains(&file_name) {
                continue;
            }

            let dir = path.parent().unwrap_or(&root_path);

            // Derive destination name from the directory.
            // Root dir → "root.md"; subdirs → "<dirname>.md".
            let dest_name = if dir == root_path {
                "root.md".to_string()
            } else {
                dir.file_name()
                    .and_then(|n| n.to_str())
                    .map(|n| format!("{n}.md"))
                    .unwrap_or_else(|| "context.md".to_string())
            };

            let dest_dir = dir.join(".normalize").join("context");
            let dest_path = dest_dir.join(&dest_name);

            let source = path
                .strip_prefix(&root_path)
                .unwrap_or(path)
                .display()
                .to_string();
            let destination = dest_path
                .strip_prefix(&root_path)
                .unwrap_or(&dest_path)
                .display()
                .to_string();

            if apply {
                std::fs::create_dir_all(&dest_dir).map_err(|e| {
                    format!("Failed to create directory {}: {e}", dest_dir.display())
                })?;

                let content = std::fs::read_to_string(path)
                    .map_err(|e| format!("Failed to read {}: {e}", path.display()))?;

                std::fs::write(&dest_path, content)
                    .map_err(|e| format!("Failed to write {}: {e}", dest_path.display()))?;

                std::fs::remove_file(path)
                    .map_err(|e| format!("Failed to remove {}: {e}", path.display()))?;
            }

            migrations.push(ContextMigrateEntry {
                source,
                destination,
            });
        }

        Ok(ContextMigrateReport {
            applied: apply,
            migrations,
        })
    }
}
