//! Local dependency discovery for language ecosystems.
//!
//! This crate defines the `LocalDeps` trait for discovering installed packages
//! on disk (node_modules, site-packages, GOPATH, cargo registry, etc.).
//!
//! This is separate from syntax analysis (`normalize-languages`) and from
//! remote registry querying (`normalize-package-index`). It answers:
//! "given this project on disk, where are the locally-installed packages
//! so we can index their symbols?"
//!
//! Only ~10 language ecosystems have real implementations. The trait provides
//! blanket defaults returning empty/None for all methods.

#[cfg(any(feature = "lang-javascript", feature = "lang-typescript"))]
pub mod ecmascript;

#[cfg(feature = "lang-c")]
pub mod c;
#[cfg(any(feature = "lang-c", feature = "lang-cpp"))]
pub mod c_cpp;
#[cfg(feature = "lang-cpp")]
pub mod cpp;
#[cfg(feature = "lang-go")]
pub mod go;
#[cfg(feature = "lang-java")]
pub mod java;
#[cfg(feature = "lang-javascript")]
pub mod javascript;
#[cfg(feature = "lang-kotlin")]
pub mod kotlin;
#[cfg(feature = "lang-python")]
pub mod python;
pub mod registry;
#[cfg(feature = "lang-rust")]
pub mod rust_lang;
#[cfg(feature = "lang-typescript")]
pub mod typescript;

use std::path::{Path, PathBuf};

/// Result of resolving an external package.
#[derive(Debug, Clone)]
pub struct ResolvedPackage {
    /// Path to the package source
    pub path: PathBuf,
    /// Package name as imported
    pub name: String,
    /// Whether this is a namespace package (no __init__.py)
    pub is_namespace: bool,
}

/// A source of local packages to index.
#[derive(Debug, Clone)]
pub struct LocalDepSource {
    /// Display name (e.g., "stdlib", "site-packages", "node_modules")
    pub name: &'static str,
    /// Path to the source directory
    pub path: PathBuf,
    /// How to traverse this source
    pub kind: LocalDepSourceKind,
    /// Whether packages here are version-specific (affects max_version in index)
    pub version_specific: bool,
}

/// How to traverse a local dependency source directory.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocalDepSourceKind {
    /// Flat directory of packages (Python site-packages, node_modules).
    /// Each top-level entry is a package.
    Flat,
    /// Recursive directory (Go stdlib, C++ includes).
    /// Packages are identified by having indexable files.
    Recursive,
    /// NPM-style scoped packages (@scope/package).
    NpmScoped,
    /// Maven repository structure (group/artifact/version).
    Maven,
    /// Gradle cache structure (group/artifact/version/hash).
    Gradle,
    /// Cargo registry structure (index/crate-version).
    Cargo,
    /// Deno cache structure (needs special handling for npm vs URL deps).
    Deno,
}

// === Helper functions for should_skip_dep_entry ===

/// Check if name is a dotfile/dotdir (starts with '.').
pub fn skip_dotfiles(name: &str) -> bool {
    name.starts_with('.')
}

/// Check if name has one of the given extensions.
pub fn has_extension(name: &str, extensions: &[&str]) -> bool {
    extensions
        .iter()
        .any(|ext| name.ends_with(&format!(".{}", ext)))
}

/// Local dependency discovery for a language ecosystem.
///
/// Discovers installed packages on the local filesystem (node_modules,
/// site-packages, cargo registry, etc.) for symbol indexing.
///
/// All methods have defaults returning empty/None. Only the ~10 language
/// ecosystems with real package management need to override them.
pub trait LocalDeps: Send + Sync {
    /// Ecosystem key (e.g., "python", "rust", "js", "go").
    /// Used as the primary identifier in the package index.
    fn ecosystem_key(&self) -> &'static str {
        ""
    }

    /// Language name (for display purposes in indexing output).
    fn language_name(&self) -> &'static str;

    /// File extensions to index when scanning packages.
    /// Required — no sensible default without knowing the language.
    fn indexable_extensions(&self) -> &'static [&'static str];

    /// Resolve a local (project-internal) import to a file path.
    ///
    /// Handles project-relative imports (e.g., `from . import foo`, `crate::`,
    /// `./module`, relative includes).
    fn resolve_local_import(
        &self,
        _import_name: &str,
        _current_file: &Path,
        _project_root: &Path,
    ) -> Option<PathBuf> {
        None
    }

    /// Resolve an external import to its source location.
    ///
    /// Returns the path to stdlib or installed packages.
    fn resolve_external_import(
        &self,
        _import_name: &str,
        _project_root: &Path,
    ) -> Option<ResolvedPackage> {
        None
    }

    /// Check if an import is from the standard library.
    fn is_stdlib_import(&self, _import_name: &str, _project_root: &Path) -> bool {
        false
    }

    /// Get the language/runtime version (for package index versioning).
    fn get_version(&self, _project_root: &Path) -> Option<String> {
        None
    }

    /// Find package cache/installation directory.
    fn find_package_cache(&self, _project_root: &Path) -> Option<PathBuf> {
        None
    }

    /// Find standard library directory (if applicable).
    /// Returns None for languages without a separate stdlib to index.
    fn find_stdlib(&self, _project_root: &Path) -> Option<PathBuf> {
        None
    }

    /// Should this entry be skipped when indexing packages?
    /// Called for each file/directory in package directories.
    fn should_skip_dep_entry(&self, name: &str, is_dir: bool) -> bool {
        if skip_dotfiles(name) {
            return true;
        }
        !is_dir && !has_extension(name, self.indexable_extensions())
    }

    /// Get the module/package name from a directory entry name.
    fn dep_module_name(&self, entry_name: &str) -> String {
        entry_name.to_string()
    }

    /// Return local dependency sources to index for this language.
    /// Each source describes a directory containing packages.
    fn dep_sources(&self, _project_root: &Path) -> Vec<LocalDepSource> {
        Vec::new()
    }

    /// Discover packages in a source directory.
    /// Returns (package_name, path) pairs for all packages found.
    fn discover_packages(&self, source: &LocalDepSource) -> Vec<(String, PathBuf)> {
        self.discover_flat_packages(&source.path)
    }

    /// Discover packages in a flat directory (each entry is a package).
    fn discover_flat_packages(&self, source_path: &Path) -> Vec<(String, PathBuf)> {
        let entries = match std::fs::read_dir(source_path) {
            Ok(e) => e,
            Err(_) => return Vec::new(),
        };

        let mut packages = Vec::new();
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();

            if self.should_skip_dep_entry(&name, path.is_dir()) {
                continue;
            }

            let module_name = self.dep_module_name(&name);
            packages.push((module_name, path));
        }
        packages
    }

    /// Discover packages recursively (each file with matching extension is a package).
    fn discover_recursive_packages(
        &self,
        base_path: &Path,
        current_path: &Path,
    ) -> Vec<(String, PathBuf)> {
        let entries = match std::fs::read_dir(current_path) {
            Ok(e) => e,
            Err(_) => return Vec::new(),
        };

        let mut packages = Vec::new();
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();
            let is_dir = path.is_dir();

            if self.should_skip_dep_entry(&name, is_dir) {
                continue;
            }

            if is_dir {
                packages.extend(self.discover_recursive_packages(base_path, &path));
            } else {
                // Get relative path from base as module name
                let rel_path = path
                    .strip_prefix(base_path)
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_else(|_| name);
                packages.push((rel_path, path));
            }
        }
        packages
    }

    /// Find the entry point file for a package path.
    /// If path is a file, returns it directly.
    /// If path is a directory, looks for language-specific entry points.
    fn find_package_entry(&self, path: &Path) -> Option<PathBuf> {
        if path.is_file() {
            Some(path.to_path_buf())
        } else {
            None
        }
    }

    /// Discover packages in npm-scoped directory (handles @scope/package).
    fn discover_npm_scoped_packages(&self, source_path: &Path) -> Vec<(String, PathBuf)> {
        let entries = match std::fs::read_dir(source_path) {
            Ok(e) => e,
            Err(_) => return Vec::new(),
        };

        let mut packages = Vec::new();
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();

            if self.should_skip_dep_entry(&name, path.is_dir()) {
                continue;
            }

            if name.starts_with('@') && path.is_dir() {
                // Scoped package - iterate contents
                if let Ok(scoped_entries) = std::fs::read_dir(&path) {
                    for scoped_entry in scoped_entries.flatten() {
                        let scoped_path = scoped_entry.path();
                        let scoped_name = scoped_entry.file_name().to_string_lossy().to_string();
                        if self.should_skip_dep_entry(&scoped_name, scoped_path.is_dir()) {
                            continue;
                        }
                        let full_name = format!("{}/{}", name, scoped_name);
                        packages.push((full_name, scoped_path));
                    }
                }
            } else {
                let module_name = self.dep_module_name(&name);
                packages.push((module_name, path));
            }
        }
        packages
    }

    /// Convert a file path to a module name for this language.
    /// Used to find "importers" — files that import a given file.
    /// Returns None for languages where this is not applicable.
    fn file_path_to_module_name(&self, _path: &Path) -> Option<String> {
        None
    }

    /// Convert a module name to candidate file paths (inverse of file_path_to_module_name).
    /// Returns relative paths that could contain the module.
    fn module_name_to_paths(&self, _module: &str) -> Vec<String> {
        Vec::new()
    }
}
