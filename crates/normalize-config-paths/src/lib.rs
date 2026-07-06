//! Shared config-slice loader for normalize verb services.
//!
//! Verb-owning crates (`normalize-graph`, `normalize-architecture`,
//! `normalize-code-similarity`, `normalize-git-history`, `normalize-facts`,
//! `normalize-filter`, `normalize-rules`, `normalize-budget`,
//! `normalize-ratchet`, …) each need to read a few sections of the global +
//! project `config.toml` **without** depending on the main crate's monolithic
//! `NormalizeConfig`. Before this crate they each hand-rolled the XDG
//! resolution + tolerant slice parsing, which drifted into six subtly different
//! (and in three cases buggy) implementations.
//!
//! This crate centralizes that logic with the **exact precedence the main
//! crate's `NormalizeConfig::load` uses**: per-section last-wins. Global config
//! (`$XDG_CONFIG_HOME/normalize/config.toml`, falling back to
//! `~/.config/normalize/config.toml`) is read first, then the project's
//! `<root>/.normalize/config.toml`. For each section, the last file that
//! declares it wins; a file that omits a section leaves the earlier file's value
//! intact. This is **not** field-level merge — an entire `[section]` from the
//! project replaces the global `[section]` when present, matching server-less's
//! `#[param(nested, serde)]` merge semantics.
//!
//! ## Why this is not a cycle
//!
//! The slice *types* (`WalkConfig`, `IndexConfig`, `PrettyConfig`,
//! `AliasConfig`) live in leaf crates (`normalize-rules-config`,
//! `normalize-index`, `normalize-output`, `normalize-filter`). This crate's
//! generic [`ConfigSlices::slice`] is parameterized over the caller's slice
//! type, so it needs **none** of them — the caller imports its own type. The
//! sole exception is the [`ConfigSlices::walk`] convenience, which applies
//! [`WalkConfig::with_daemon_baseline`] and therefore depends on
//! `normalize-rules-config` (a low leaf that never depends back). `[analyze]`
//! parsing stays with each caller (its rich `AnalyzeSlice` types differ), so the
//! main crate's `AnalyzeConfig` is never referenced here.

use normalize_rules_config::WalkConfig;
use serde::de::DeserializeOwned;
use std::path::{Path, PathBuf};

/// Ordered list of `config.toml` paths that **exist**, global (XDG) first then
/// project (`<root>/.normalize/config.toml`).
///
/// The global path is `$XDG_CONFIG_HOME/normalize/config.toml` when the env var
/// is set, else `~/.config/normalize/config.toml`. Nonexistent paths are
/// filtered out, so the returned vec contains only files the caller can read.
pub fn config_paths(root: &Path) -> Vec<PathBuf> {
    let global = std::env::var("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .ok()
        .or_else(|| dirs::home_dir().map(|h| h.join(".config")))
        .map(|c| c.join("normalize").join("config.toml"));
    [global, Some(root.join(".normalize").join("config.toml"))]
        .into_iter()
        .flatten()
        .filter(|p| p.exists())
        .collect()
}

/// Config sections read from the global then project `config.toml`, with
/// per-section last-wins precedence (project overrides global), matching the
/// main crate's `NormalizeConfig::load`.
///
/// Constructed once per command via [`ConfigSlices::load`]; the files are read
/// and parsed a single time regardless of how many slices the caller extracts.
#[derive(Default)]
pub struct ConfigSlices {
    /// Parsed tables in precedence order: global first, then project. Only
    /// files that existed *and* parsed as TOML are retained (unparseable files
    /// are skipped tolerantly, as the per-crate loaders this replaces did).
    tables: Vec<toml::Table>,
}

impl ConfigSlices {
    /// Read the global then project `config.toml`, retaining each that exists
    /// and parses. Missing or malformed files are skipped (tolerant), never an
    /// error — a command with no config gets `T::default()` slices.
    pub fn load(root: &Path) -> Self {
        let mut tables = Vec::new();
        for path in config_paths(root) {
            if let Ok(content) = std::fs::read_to_string(&path)
                && let Ok(table) = content.parse::<toml::Table>()
            {
                tables.push(table);
            }
        }
        Self { tables }
    }

    /// Deserialize one config `[section]` with per-section last-wins precedence.
    ///
    /// Returns `T::default()` when no file declares the section. When a file
    /// declares `[section]` but it fails to deserialize into `T`, that file is
    /// skipped and the previous value is kept — mirroring the tolerant
    /// `if let Ok(parsed)` behavior of the verb loaders this replaces. Because
    /// the whole section is replaced (not merged field-by-field), this matches
    /// server-less's `#[param(nested, serde)]` semantics used by the main crate.
    pub fn slice<T: DeserializeOwned + Default>(&self, section: &str) -> T {
        let mut value = T::default();
        for table in &self.tables {
            if let Some(sub) = table.get(section)
                && let Ok(parsed) = sub.clone().try_into::<T>()
            {
                value = parsed;
            }
        }
        value
    }

    /// The `[walk]` slice with the daemon baseline applied.
    ///
    /// Convenience over `self.slice::<WalkConfig>("walk").with_daemon_baseline()`.
    /// Always excludes `.git/` and `.normalize/` even when no `[walk]` section is
    /// present, so index walkers never descend into `.normalize/` (where
    /// `index.sqlite` lives) and spin. Used by `normalize-facts` and
    /// `normalize-rules`, which previously each hand-rolled this fallback.
    pub fn walk(&self) -> WalkConfig {
        self.slice::<WalkConfig>("walk").with_daemon_baseline()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[derive(serde::Deserialize, Default, Debug, PartialEq)]
    #[serde(default)]
    struct DemoSlice {
        enabled: bool,
        name: String,
    }

    /// Set `$XDG_CONFIG_HOME` for the closure so the global path is deterministic
    /// and isolated from the developer's real `~/.config`.
    fn with_global<R>(dir: &Path, f: impl FnOnce() -> R) -> R {
        // Tests touch a process-global env var; this module is single-threaded
        // per test binary invocation here, but guard against interleaving.
        static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
        let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let prev = std::env::var_os("XDG_CONFIG_HOME");
        unsafe { std::env::set_var("XDG_CONFIG_HOME", dir) };
        let r = f();
        match prev {
            Some(v) => unsafe { std::env::set_var("XDG_CONFIG_HOME", v) },
            None => unsafe { std::env::remove_var("XDG_CONFIG_HOME") },
        }
        r
    }

    fn write(path: &Path, contents: &str) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, contents).unwrap();
    }

    #[test]
    fn project_section_overrides_global() {
        let xdg = TempDir::new().unwrap();
        let root = TempDir::new().unwrap();
        write(
            &xdg.path().join("normalize").join("config.toml"),
            "[demo]\nenabled = false\nname = \"global\"\n",
        );
        write(
            &root.path().join(".normalize").join("config.toml"),
            "[demo]\nname = \"project\"\n",
        );
        let slice: DemoSlice =
            with_global(xdg.path(), || ConfigSlices::load(root.path()).slice("demo"));
        // Whole-section replace: project's [demo] wins entirely, so `enabled`
        // reverts to its default (false here) — NOT field-level merge.
        assert_eq!(
            slice,
            DemoSlice {
                enabled: false,
                name: "project".into()
            }
        );
    }

    #[test]
    fn global_kept_when_project_omits_section() {
        let xdg = TempDir::new().unwrap();
        let root = TempDir::new().unwrap();
        write(
            &xdg.path().join("normalize").join("config.toml"),
            "[demo]\nenabled = true\nname = \"global\"\n",
        );
        // Project config exists but declares a DIFFERENT section — the bug this
        // fixes: it must NOT reset the global [demo].
        write(
            &root.path().join(".normalize").join("config.toml"),
            "[other]\nx = 1\n",
        );
        let slice: DemoSlice =
            with_global(xdg.path(), || ConfigSlices::load(root.path()).slice("demo"));
        assert_eq!(
            slice,
            DemoSlice {
                enabled: true,
                name: "global".into()
            }
        );
    }

    #[test]
    fn default_when_absent_and_walk_baseline() {
        let xdg = TempDir::new().unwrap();
        let root = TempDir::new().unwrap();
        let (slice, walk): (DemoSlice, WalkConfig) = with_global(xdg.path(), || {
            let s = ConfigSlices::load(root.path());
            (s.slice("demo"), s.walk())
        });
        assert_eq!(slice, DemoSlice::default());
        // walk() always carries the daemon baseline even with no config at all.
        let ex = walk.exclude.unwrap_or_default();
        assert!(ex.contains(&".git/".to_string()));
        assert!(ex.contains(&".normalize/".to_string()));
    }
}
