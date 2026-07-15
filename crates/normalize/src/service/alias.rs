//! `normalize alias save` — capture the previously-run (or explicitly given)
//! normalize command as a named `@alias`.
//!
//! Complements the read side of the unified alias system (`normalize aliases`
//! to list, `normalize @name` to run) with a write side. By default it reads
//! the state file `.normalize/.last-command`, written after every successful
//! invocation by `main.rs`'s [`record_last_command`], so a typical session
//! looks like:
//!
//! ```text
//! normalize rank complexity --root src/
//! normalize alias save complexity-src
//! normalize @complexity-src
//! ```
//!
//! `--command` overrides the recorded last command for scripted/agent use
//! where there is no prior invocation to recall.

use crate::filter::{AliasEntry, AliasSyntax, AliasValue};
use crate::output::OutputFormatter;
use schemars::JsonSchema;
use serde::Serialize;
use std::cell::Cell;
use std::io::IsTerminal;
use std::path::{Path, PathBuf};

/// Name of the state file (inside `.normalize/`) that records the argv of the
/// last successfully-dispatched normalize command. Written by `main.rs`.
pub const LAST_COMMAND_FILE: &str = ".last-command";

/// Read the last-recorded command for `root`. Returns `None` if no command
/// has been recorded yet (fresh project, or the file was never written).
pub fn read_last_command(root: &Path) -> Option<String> {
    let path = root.join(".normalize").join(LAST_COMMAND_FILE);
    std::fs::read_to_string(path)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Record `command` as the last-run normalize command for `root`.
///
/// Best-effort: failures (read-only filesystem, missing permissions) are
/// swallowed since this is convenience state, not part of any command's
/// success/failure contract.
pub fn write_last_command(root: &Path, command: &str) {
    let dir = root.join(".normalize");
    if std::fs::create_dir_all(&dir).is_ok() {
        let _ = std::fs::write(dir.join(LAST_COMMAND_FILE), command);
    }
}

/// Report from `normalize alias save`.
#[derive(Debug, Serialize, JsonSchema)]
pub struct AliasSaveReport {
    /// The alias name (without the `@` prefix).
    pub name: String,
    /// The syntax that was written (declared explicitly, never inferred silently).
    pub syntax: String,
    /// The command/value that was saved.
    pub value: String,
    /// Path to the config file the alias was written into.
    pub config_path: String,
    /// True if an existing alias with the same name was overwritten.
    pub overwritten: bool,
}

impl OutputFormatter for AliasSaveReport {
    fn format_text(&self) -> String {
        let verb = if self.overwritten {
            "Overwrote"
        } else {
            "Saved"
        };
        format!(
            "{verb} alias @{} (syntax: {}) = '{}'\n  in {}",
            self.name, self.syntax, self.value, self.config_path
        )
    }
}

/// Prompt for an optional description on an interactive TTY. Returns `None`
/// when stdin isn't a terminal (scripts/agents/CI) or the user enters nothing
/// — non-interactive callers must pass `--description` instead.
fn prompt_description() -> Option<String> {
    if !std::io::stdin().is_terminal() {
        return None;
    }
    use std::io::Write as _;
    eprint!("Description (optional): ");
    let _ = std::io::stderr().flush();
    let mut line = String::new();
    std::io::stdin().read_line(&mut line).ok()?;
    let trimmed = line.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn default_config_file(root: &Path) -> PathBuf {
    root.join(".normalize").join("config.toml")
}

/// Ensure `doc["aliases"]` exists as a table and return it.
fn ensure_aliases_table(doc: &mut toml_edit::DocumentMut) -> Result<&mut toml_edit::Table, String> {
    if !doc.contains_key("aliases") {
        let mut t = toml_edit::Table::new();
        t.set_implicit(true);
        doc["aliases"] = toml_edit::Item::Table(t);
    }
    doc["aliases"]
        .as_table_mut()
        .ok_or_else(|| "'aliases' is not a table in the config file".to_string())
}

/// Alias management service (`normalize alias ...`).
pub struct AliasService {
    pretty: Cell<bool>,
    pretty_raw: Cell<bool>,
    compact_raw: Cell<bool>,
}

impl AliasService {
    pub fn new(pretty: &Cell<bool>) -> Self {
        Self {
            pretty: Cell::new(pretty.get()),
            pretty_raw: Cell::new(false),
            compact_raw: Cell::new(false),
        }
    }

    fn display_save(&self, r: &AliasSaveReport) -> String {
        if self.pretty.get() {
            r.format_pretty()
        } else {
            r.format_text()
        }
    }

    fn resolve_root(root: Option<String>) -> Result<PathBuf, String> {
        root.map(PathBuf::from)
            .map(Ok)
            .unwrap_or_else(std::env::current_dir)
            .map_err(|e| format!("Failed to get current directory: {e}"))
    }

    fn resolve_format(&self, root: &Path) {
        self.pretty.set(super::resolve_pretty(
            root,
            self.pretty_raw.get(),
            self.compact_raw.get(),
        ));
    }
}

impl server_less::CliGlobals for AliasService {
    fn set_global_flag(&self, name: &str, value: bool) {
        match name {
            "pretty" => self.pretty_raw.set(value),
            "compact" => self.compact_raw.set(value),
            _ => {}
        }
    }
}

#[server_less::cli(
    name = "alias",
    description = "Save the previously-run normalize command as a named alias"
)]
impl AliasService {
    /// Save the last-run (or explicitly given) normalize command as a named `@alias`.
    ///
    /// By default reads `.normalize/.last-command` (recorded after every successful
    /// invocation); `--command` overrides it. Writes `[aliases.<name>]` into the
    /// project's `.normalize/config.toml`, creating the file if needed and preserving
    /// all existing content. Fails if the alias name already exists unless `--force`.
    ///
    /// Examples:
    ///   normalize rank complexity --root src/
    ///   normalize alias save complexity-src                 # captures the command above
    ///   normalize alias save vocab --command 'structure query "SELECT 1"'
    ///   normalize alias save my-alias --description "..." --force
    #[allow(clippy::too_many_arguments)]
    #[cli(display_with = "display_save")]
    pub fn save(
        &self,
        #[param(positional, help = "Alias name (without the @ prefix)")] name: String,
        #[param(help = "Command to save (default: the last-run normalize command)")]
        command: Option<String>,
        #[param(help = "Human-readable description")] description: Option<String>,
        #[param(help = "Overwrite the alias if it already exists")] force: bool,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
    ) -> Result<AliasSaveReport, String> {
        let root_path = Self::resolve_root(root)?;
        self.resolve_format(&root_path);

        if name.is_empty() {
            return Err("alias name must not be empty".to_string());
        }
        if let Some(stripped) = name.strip_prefix('@') {
            return Err(format!(
                "alias name must not include the '@' prefix (use '{stripped}')"
            ));
        }

        let value = match command {
            Some(c) if !c.trim().is_empty() => c,
            _ => read_last_command(&root_path).ok_or_else(|| {
                "no previous command recorded — run a normalize command first, or pass --command"
                    .to_string()
            })?,
        };

        // Infer syntax with the same heuristic used everywhere else in the
        // alias system, then write it explicitly so the saved alias never
        // triggers the "missing syntax" warning at load time.
        let probe = AliasEntry {
            syntax: None,
            value: AliasValue::Single(value.clone()),
            description: None,
        };
        let syntax: AliasSyntax = probe.resolved_syntax();

        let config_path = default_config_file(&root_path);
        if let Some(parent) = config_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("cannot create '{}': {e}", parent.display()))?;
        }
        let existing = std::fs::read_to_string(&config_path).unwrap_or_default();
        let mut doc: toml_edit::DocumentMut = existing
            .parse()
            .map_err(|e| format!("TOML parse error in '{}': {e}", config_path.display()))?;

        let overwritten = doc
            .get("aliases")
            .and_then(|a| a.as_table())
            .is_some_and(|t| t.contains_key(&name));
        if overwritten && !force {
            return Err(format!(
                "alias @{name} already exists in '{}' (pass --force to overwrite)",
                config_path.display()
            ));
        }

        let description = description.or_else(prompt_description);

        let mut entry_table = toml_edit::Table::new();
        entry_table["syntax"] = toml_edit::value(syntax.to_string());
        entry_table["value"] = toml_edit::value(value.clone());
        if let Some(ref d) = description {
            entry_table["description"] = toml_edit::value(d.clone());
        }

        let aliases_table = ensure_aliases_table(&mut doc)?;
        aliases_table[&name] = toml_edit::Item::Table(entry_table);

        std::fs::write(&config_path, doc.to_string())
            .map_err(|e| format!("cannot write '{}': {e}", config_path.display()))?;

        Ok(AliasSaveReport {
            name,
            syntax: syntax.to_string(),
            value,
            config_path: config_path.to_string_lossy().into_owned(),
            overwritten,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_read_write_last_command_roundtrip() {
        let dir = TempDir::new().unwrap_or_else(|e| panic!("tempdir: {e}"));
        assert_eq!(read_last_command(dir.path()), None);
        write_last_command(dir.path(), "rank complexity --root src/");
        assert_eq!(
            read_last_command(dir.path()),
            Some("rank complexity --root src/".to_string())
        );
    }

    #[test]
    fn test_save_writes_new_alias() {
        let dir = TempDir::new().unwrap_or_else(|e| panic!("tempdir: {e}"));
        let pretty = Cell::new(false);
        let service = AliasService::new(&pretty);

        let report = service
            .save(
                "complexity-src".to_string(),
                Some("rank complexity --root src/".to_string()),
                Some("Complexity for src".to_string()),
                false,
                Some(dir.path().to_string_lossy().into_owned()),
            )
            .unwrap_or_else(|e| panic!("save failed: {e}"));

        assert_eq!(report.name, "complexity-src");
        assert_eq!(report.syntax, "command");
        assert_eq!(report.value, "rank complexity --root src/");
        assert!(!report.overwritten);

        let content = std::fs::read_to_string(dir.path().join(".normalize/config.toml"))
            .unwrap_or_else(|e| panic!("read config: {e}"));
        assert!(content.contains("[aliases.complexity-src]"));
        assert!(content.contains("syntax = \"command\""));
        assert!(content.contains("rank complexity --root src/"));
        assert!(content.contains("Complexity for src"));
    }

    #[test]
    fn test_save_preserves_existing_content() {
        let dir = TempDir::new().unwrap_or_else(|e| panic!("tempdir: {e}"));
        let normalize_dir = dir.path().join(".normalize");
        std::fs::create_dir_all(&normalize_dir).unwrap_or_else(|e| panic!("mkdir: {e}"));
        std::fs::write(
            normalize_dir.join("config.toml"),
            "[daemon]\nenabled = false\n\n[aliases.existing]\nsyntax = \"glob\"\nvalue = [\"*.rs\"]\n",
        )
        .unwrap_or_else(|e| panic!("write config: {e}"));

        let pretty = Cell::new(false);
        let service = AliasService::new(&pretty);
        service
            .save(
                "new-one".to_string(),
                Some("view src/".to_string()),
                None,
                false,
                Some(dir.path().to_string_lossy().into_owned()),
            )
            .unwrap_or_else(|e| panic!("save failed: {e}"));

        let content = std::fs::read_to_string(normalize_dir.join("config.toml"))
            .unwrap_or_else(|e| panic!("read config: {e}"));
        assert!(content.contains("[daemon]"));
        assert!(content.contains("enabled = false"));
        assert!(content.contains("[aliases.existing]"));
        assert!(content.contains("*.rs"));
        assert!(content.contains("[aliases.new-one]"));
    }

    #[test]
    fn test_save_rejects_duplicate_without_force() {
        let dir = TempDir::new().unwrap_or_else(|e| panic!("tempdir: {e}"));
        let pretty = Cell::new(false);
        let service = AliasService::new(&pretty);
        service
            .save(
                "dup".to_string(),
                Some("view src/".to_string()),
                None,
                false,
                Some(dir.path().to_string_lossy().into_owned()),
            )
            .unwrap_or_else(|e| panic!("first save failed: {e}"));

        let err = service
            .save(
                "dup".to_string(),
                Some("view lib/".to_string()),
                None,
                false,
                Some(dir.path().to_string_lossy().into_owned()),
            )
            .unwrap_err();
        assert!(err.contains("already exists"), "got: {err}");

        // With --force it succeeds and overwrites.
        let report = service
            .save(
                "dup".to_string(),
                Some("view lib/".to_string()),
                None,
                true,
                Some(dir.path().to_string_lossy().into_owned()),
            )
            .unwrap_or_else(|e| panic!("forced save failed: {e}"));
        assert!(report.overwritten);
        assert_eq!(report.value, "view lib/");
    }

    #[test]
    fn test_save_no_command_and_no_history_errors() {
        let dir = TempDir::new().unwrap_or_else(|e| panic!("tempdir: {e}"));
        let pretty = Cell::new(false);
        let service = AliasService::new(&pretty);
        let err = service
            .save(
                "nope".to_string(),
                None,
                None,
                false,
                Some(dir.path().to_string_lossy().into_owned()),
            )
            .unwrap_err();
        assert!(err.contains("no previous command"), "got: {err}");
    }

    #[test]
    fn test_save_falls_back_to_last_command_file() {
        let dir = TempDir::new().unwrap_or_else(|e| panic!("tempdir: {e}"));
        write_last_command(dir.path(), "grep TODO --only \"*.rs\"");

        let pretty = Cell::new(false);
        let service = AliasService::new(&pretty);
        let report = service
            .save(
                "find-todos".to_string(),
                None,
                None,
                false,
                Some(dir.path().to_string_lossy().into_owned()),
            )
            .unwrap_or_else(|e| panic!("save failed: {e}"));
        assert_eq!(report.value, "grep TODO --only \"*.rs\"");
    }

    #[test]
    fn test_save_rejects_at_prefix() {
        let dir = TempDir::new().unwrap_or_else(|e| panic!("tempdir: {e}"));
        let pretty = Cell::new(false);
        let service = AliasService::new(&pretty);
        let err = service
            .save(
                "@nope".to_string(),
                Some("view src/".to_string()),
                None,
                false,
                Some(dir.path().to_string_lossy().into_owned()),
            )
            .unwrap_err();
        assert!(err.contains("'@' prefix"), "got: {err}");
    }
}
