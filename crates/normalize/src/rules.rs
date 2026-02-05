//! Rule pack loading and execution.
//!
//! This module handles discovering, loading, and running rule packs.
//! Rule packs are dylibs that implement the RulePack interface.

use abi_stable::library::{LibraryError, RootModule};
use normalize_facts_rules_api::{Diagnostic, Relations, RulePackInfo, RulePackRef};
use std::path::{Path, PathBuf};

/// A loaded rule pack
pub struct LoadedRulePack {
    /// The loaded library reference
    pack: RulePackRef,
    /// Path to the dylib (for display)
    pub path: PathBuf,
}

impl LoadedRulePack {
    /// Get information about this rule pack
    pub fn info(&self) -> RulePackInfo {
        (self.pack.info())()
    }

    /// Run all rules in this pack
    pub fn run(&self, relations: &Relations) -> Vec<Diagnostic> {
        (self.pack.run())(relations).into_iter().collect()
    }

    /// Run a specific rule by ID
    pub fn run_rule(&self, rule_id: &str, relations: &Relations) -> Vec<Diagnostic> {
        (self.pack.run_rule())(rule_id.into(), relations)
            .into_iter()
            .collect()
    }
}

/// Error type for rule pack loading
#[derive(Debug)]
pub enum RulePackError {
    /// Library loading failed
    Load(LibraryError),
    /// Library not found at path
    NotFound(PathBuf),
    /// Invalid library (not a rule pack)
    Invalid(String),
}

impl std::fmt::Display for RulePackError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RulePackError::Load(e) => write!(f, "Failed to load rule pack: {}", e),
            RulePackError::NotFound(p) => write!(f, "Rule pack not found: {}", p.display()),
            RulePackError::Invalid(s) => write!(f, "Invalid rule pack: {}", s),
        }
    }
}

impl std::error::Error for RulePackError {}

/// Load a rule pack from a specific path
pub fn load_from_path(path: &Path) -> Result<LoadedRulePack, RulePackError> {
    if !path.exists() {
        return Err(RulePackError::NotFound(path.to_path_buf()));
    }

    let pack = RulePackRef::load_from_file(path).map_err(RulePackError::Load)?;

    Ok(LoadedRulePack {
        pack,
        path: path.to_path_buf(),
    })
}

/// Get the library extension for the current platform
fn lib_extension() -> &'static str {
    if cfg!(target_os = "windows") {
        "dll"
    } else if cfg!(target_os = "macos") {
        "dylib"
    } else {
        "so"
    }
}

/// Get the library prefix for the current platform
fn lib_prefix() -> &'static str {
    if cfg!(target_os = "windows") {
        ""
    } else {
        "lib"
    }
}

/// Search paths for rule packs
pub fn search_paths(root: &Path) -> Vec<PathBuf> {
    let mut paths = Vec::new();

    // Project-local rules
    paths.push(root.join(".normalize/rules"));

    // User rules
    if let Some(home) = dirs::home_dir() {
        paths.push(home.join(".normalize/rules"));
    }

    // XDG data dir
    if let Some(data_dir) = dirs::data_dir() {
        paths.push(data_dir.join("normalize/rules"));
    }

    paths
}

/// Discover all rule packs in search paths
pub fn discover(root: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let ext = lib_extension();
    let prefix = lib_prefix();

    for search_path in search_paths(root) {
        if !search_path.exists() {
            continue;
        }

        if let Ok(entries) = std::fs::read_dir(&search_path) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file()
                    && let Some(name) = path.file_name().and_then(|n| n.to_str())
                    && name.starts_with(prefix)
                    && name.ends_with(ext)
                {
                    found.push(path);
                }
            }
        }
    }

    found
}

/// Load all discovered rule packs
pub fn load_all(root: &Path) -> Vec<Result<LoadedRulePack, RulePackError>> {
    discover(root)
        .into_iter()
        .map(|path| load_from_path(&path))
        .collect()
}

/// Format a diagnostic for display
pub fn format_diagnostic(diag: &Diagnostic, use_colors: bool) -> String {
    use normalize_facts_rules_api::DiagnosticLevel;

    let level_str = match diag.level {
        DiagnosticLevel::Hint => {
            if use_colors {
                "\x1b[36mhint\x1b[0m"
            } else {
                "hint"
            }
        }
        DiagnosticLevel::Warning => {
            if use_colors {
                "\x1b[33mwarning\x1b[0m"
            } else {
                "warning"
            }
        }
        DiagnosticLevel::Error => {
            if use_colors {
                "\x1b[31merror\x1b[0m"
            } else {
                "error"
            }
        }
    };

    let mut out = String::new();

    // Location
    if let abi_stable::std_types::ROption::RSome(ref loc) = diag.location {
        out.push_str(&format!("{}:{}: ", loc.file, loc.line));
    }

    // Level and rule
    out.push_str(&format!("{} [{}]: ", level_str, diag.rule_id));

    // Message
    out.push_str(&diag.message);

    // Related locations
    for related in diag.related.iter() {
        out.push_str(&format!("\n  --> {}:{}", related.file, related.line));
    }

    // Suggestion
    if let abi_stable::std_types::ROption::RSome(ref suggestion) = diag.suggestion {
        out.push_str(&format!("\n  suggestion: {}", suggestion));
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_search_paths() {
        let paths = search_paths(Path::new("/tmp/test"));
        assert!(!paths.is_empty());
        assert!(paths[0].ends_with(".normalize/rules"));
    }

    #[test]
    fn test_lib_extension() {
        let ext = lib_extension();
        assert!(ext == "so" || ext == "dylib" || ext == "dll");
    }
}
