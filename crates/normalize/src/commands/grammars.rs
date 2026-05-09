//! Grammar management commands.

use crate::output::OutputFormatter;
use serde::Serialize;
use std::io::IsTerminal;
use std::path::{Path, PathBuf};

/// A single grammar entry with its name and file path.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct GrammarEntry {
    pub name: String,
    pub path: String,
}

/// Grammar list report
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct GrammarListReport {
    grammars: Vec<GrammarEntry>,
}

impl GrammarListReport {
    pub fn new(grammars: Vec<(String, PathBuf)>) -> Self {
        Self {
            grammars: grammars
                .into_iter()
                .map(|(name, path)| GrammarEntry {
                    name,
                    path: path.display().to_string(),
                })
                .collect(),
        }
    }
}

impl OutputFormatter for GrammarListReport {
    fn format_text(&self) -> String {
        if self.grammars.is_empty() {
            let mut lines = vec!["No grammars installed.".to_string(), String::new()];
            lines.push("Install grammars with: normalize grammars install".to_string());
            lines.push(
                "Or set NORMALIZE_GRAMMAR_PATH to a directory containing .so/.dylib files"
                    .to_string(),
            );
            lines.join("\n")
        } else {
            let mut lines = vec![format!("Installed grammars ({}):", self.grammars.len())];
            for entry in &self.grammars {
                lines.push(entry.name.clone());
            }
            lines.join("\n")
        }
    }
}

/// Grammar path item
#[derive(Debug, Serialize, schemars::JsonSchema)]
struct GrammarPath {
    source: String,
    path: String,
    exists: bool,
}

/// Grammar paths report
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct GrammarPathsReport {
    paths: Vec<GrammarPath>,
}

impl OutputFormatter for GrammarPathsReport {
    fn format_text(&self) -> String {
        let mut lines = vec!["Grammar search paths:".to_string()];
        for item in &self.paths {
            let exists = if item.exists { "" } else { " (not found)" };
            lines.push(format!("  [{}] {}{}", item.source, item.path, exists));
        }
        lines.join("\n")
    }
}

/// Stamp file written to the user's grammar install directory once we've
/// either downloaded grammars on the user's behalf, or confirmed an existing
/// install. Its presence is the signal "we've checked; don't auto-install
/// again on every command." Format: a single line containing the version (or
/// `prebuilt` when the dir was already populated by `xtask build-grammars`).
const INSTALLED_STAMP: &str = ".installed-version";

/// Where grammars are auto-installed for the current user.
fn user_grammars_dir() -> Option<PathBuf> {
    dirs::config_dir().map(|c| c.join("normalize/grammars"))
}

/// Returns true if the user grammars directory has at least one `.so`/`.dylib`/`.dll`.
fn dir_has_grammars(dir: &Path) -> bool {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return false;
    };
    entries.flatten().any(|e| {
        e.path()
            .extension()
            .and_then(|s| s.to_str())
            .is_some_and(|ext| matches!(ext, "so" | "dylib" | "dll"))
    })
}

/// Mark the user grammar install dir as "checked" so we don't re-attempt
/// auto-install on every invocation. Best-effort; failures are silent because
/// the worst case is "we re-check next time."
fn write_installed_stamp(dir: &Path, marker: &str) {
    let _ = std::fs::create_dir_all(dir);
    let _ = std::fs::write(dir.join(INSTALLED_STAMP), marker);
}

/// Outcome of a first-run grammar check, reported back to the caller so it
/// can decide whether to proceed (e.g. `init` may want to print a notice).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GrammarsFirstRun {
    /// Stamp file existed; nothing to do.
    AlreadyChecked,
    /// Grammars were already in place (e.g. from a workspace build); we
    /// only wrote the stamp file so future invocations short-circuit.
    PreInstalled,
    /// We auto-installed grammars on the user's behalf (non-TTY mode).
    AutoInstalled { count: usize },
    /// Grammars are missing and we did not install (TTY mode — leave it to
    /// the user / `init` to prompt).
    SkippedInteractive,
    /// Auto-install was attempted but failed; the user will see the usual
    /// "run `normalize grammars install`" warning later.
    Failed { error: String },
    /// User config dir could not be determined.
    NoConfigDir,
}

/// Lightweight first-use grammar check.
///
/// Behaviour:
/// 1. If the install stamp exists **and its version matches the running
///    binary**, returns immediately.
///    If the stamp is present but records a different version (e.g. after
///    `normalize update`), the stamp is deleted and we fall through to
///    re-download grammars for the current binary version.
/// 2. Otherwise, if the grammar dir already contains shared libraries (e.g.
///    a developer ran `cargo xtask build-grammars`), write the stamp and
///    return `PreInstalled`.
/// 3. Otherwise, in non-interactive sessions auto-install with a stderr
///    notice.
/// 4. Otherwise (TTY), do nothing — `normalize init` and missing-grammar
///    warnings will guide the user.
///
/// This is meant to be called at most once per process, before any command
/// that needs grammars runs. Calling it on `--help`, `--version`, etc. is
/// harmless but pointless.
pub fn ensure_grammars_first_use() -> GrammarsFirstRun {
    let Some(dir) = user_grammars_dir() else {
        return GrammarsFirstRun::NoConfigDir;
    };

    let stamp_path = dir.join(INSTALLED_STAMP);
    if stamp_path.exists() {
        // Read the stamp and compare against the running binary's version.
        // If they match (or the stamp says "prebuilt", which is a local
        // developer build not tied to any release), we're done.
        let stamp_content = std::fs::read_to_string(&stamp_path).unwrap_or_default();
        let stamp_version = stamp_content.trim();
        let binary_version = env!("CARGO_PKG_VERSION");

        if stamp_version == "prebuilt" || stamp_version == binary_version {
            return GrammarsFirstRun::AlreadyChecked;
        }

        // Version mismatch — the binary was updated but grammars were not.
        // Delete the stale stamp so we fall through and reinstall.
        eprintln!(
            "Grammar version mismatch (installed: {}, binary: {}) — reinstalling grammars",
            stamp_version, binary_version
        );
        let _ = std::fs::remove_file(&stamp_path);
    }

    // If a NORMALIZE_GRAMMAR_PATH is set and points at a populated directory,
    // treat that as a pre-install too — write the stamp so we don't pester.
    let env_has_grammars = std::env::var("NORMALIZE_GRAMMAR_PATH")
        .ok()
        .map(|p| {
            p.split(':')
                .filter(|s| !s.is_empty())
                .any(|p| dir_has_grammars(Path::new(p)))
        })
        .unwrap_or(false);

    if env_has_grammars || dir_has_grammars(&dir) {
        write_installed_stamp(&dir, "prebuilt\n");
        return GrammarsFirstRun::PreInstalled;
    }

    // Non-TTY: auto-install. TTY: leave it for the user / init wizard.
    if std::io::stdin().is_terminal() {
        return GrammarsFirstRun::SkippedInteractive;
    }

    eprintln!("Auto-installing grammars on first use...");
    let pretty = std::cell::Cell::new(false);
    let service = crate::service::grammars::GrammarService::new(&pretty);
    // Pin to the binary's own version. Grammars from a different release may
    // have ABI/extraction changes incompatible with this binary; falling back
    // to `releases/latest` would silently install the wrong artifact for any
    // user not on the latest version.
    let pinned_version = format!("v{}", env!("CARGO_PKG_VERSION"));
    match service.install(Some(pinned_version), false, false) {
        Ok(report) => {
            let version = report.version.clone().unwrap_or_else(|| "unknown".into());
            write_installed_stamp(&dir, &format!("{version}\n"));
            GrammarsFirstRun::AutoInstalled {
                count: report.count,
            }
        }
        Err(error) => GrammarsFirstRun::Failed { error },
    }
}

/// Build a grammar paths report (shared with the service layer).
pub fn build_paths_report() -> GrammarPathsReport {
    let mut raw_paths = Vec::new();

    // Environment variable
    if let Ok(env_path) = std::env::var("NORMALIZE_GRAMMAR_PATH") {
        for p in env_path.split(':') {
            if !p.is_empty() {
                raw_paths.push(("env", PathBuf::from(p)));
            }
        }
    }

    // User config directory
    if let Some(config) = dirs::config_dir() {
        raw_paths.push(("config", config.join("normalize/grammars")));
    }

    let paths: Vec<GrammarPath> = raw_paths
        .iter()
        .map(|(source, path)| GrammarPath {
            source: source.to_string(),
            path: path.display().to_string(),
            exists: path.exists(),
        })
        .collect();

    GrammarPathsReport { paths }
}
