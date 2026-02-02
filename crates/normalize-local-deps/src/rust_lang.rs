//! Rust local dependency discovery.

use crate::{
    LocalDepSource, LocalDepSourceKind, LocalDeps, ResolvedPackage, has_extension, skip_dotfiles,
};
use std::path::{Path, PathBuf};
use std::process::Command;

// ============================================================================
// Helper functions (moved from normalize-languages/src/rust.rs)
// ============================================================================

/// Get Rust version.
pub fn get_rust_version() -> Option<String> {
    let output = Command::new("rustc").args(["--version"]).output().ok()?;

    if output.status.success() {
        let version_str = String::from_utf8_lossy(&output.stdout);
        // "rustc 1.75.0 (82e1608df 2023-12-21)" -> "1.75"
        for part in version_str.split_whitespace() {
            if part.chars().next().is_some_and(|c| c.is_ascii_digit()) {
                let parts: Vec<&str> = part.split('.').collect();
                if parts.len() >= 2 {
                    return Some(format!("{}.{}", parts[0], parts[1]));
                }
            }
        }
    }

    None
}

/// Find cargo registry source directory.
/// Structure: ~/.cargo/registry/src/
pub fn find_cargo_registry() -> Option<PathBuf> {
    // Check CARGO_HOME env var
    if let Ok(cargo_home) = std::env::var("CARGO_HOME") {
        let registry = PathBuf::from(cargo_home).join("registry").join("src");
        if registry.is_dir() {
            return Some(registry);
        }
    }

    // Fall back to ~/.cargo/registry/src
    if let Ok(home) = std::env::var("HOME") {
        let registry = PathBuf::from(home)
            .join(".cargo")
            .join("registry")
            .join("src");
        if registry.is_dir() {
            return Some(registry);
        }
    }

    // Windows fallback
    if let Ok(home) = std::env::var("USERPROFILE") {
        let registry = PathBuf::from(home)
            .join(".cargo")
            .join("registry")
            .join("src");
        if registry.is_dir() {
            return Some(registry);
        }
    }

    None
}

/// Resolve a Rust crate import to its source location.
fn resolve_rust_crate(crate_name: &str, registry: &Path) -> Option<ResolvedPackage> {
    // Registry structure: registry/src/index.crates.io-*/crate-version/
    if let Ok(indices) = std::fs::read_dir(registry) {
        for index_entry in indices.flatten() {
            let index_path = index_entry.path();
            if !index_path.is_dir() {
                continue;
            }

            if let Ok(crates) = std::fs::read_dir(&index_path) {
                for crate_entry in crates.flatten() {
                    let crate_dir = crate_entry.path();
                    let dir_name = crate_entry.file_name().to_string_lossy().to_string();

                    // Check if this is our crate (name-version pattern)
                    if dir_name.starts_with(&format!("{}-", crate_name)) {
                        let lib_rs = crate_dir.join("src").join("lib.rs");
                        if lib_rs.is_file() {
                            return Some(ResolvedPackage {
                                path: lib_rs,
                                name: crate_name.to_string(),
                                is_namespace: false,
                            });
                        }
                    }
                }
            }
        }
    }

    None
}

/// Discover packages in Cargo registry structure.
/// Structure: ~/.cargo/registry/src/index.crates.io-*/crate-version/
fn discover_cargo_packages(registry: &Path) -> Vec<(String, PathBuf)> {
    let mut packages = Vec::new();

    // Registry structure: registry/src/index.crates.io-*/crate-version/
    let indices = match std::fs::read_dir(registry) {
        Ok(e) => e,
        Err(_) => return packages,
    };

    for index_entry in indices.flatten() {
        let index_path = index_entry.path();
        if !index_path.is_dir() {
            continue;
        }

        let crates = match std::fs::read_dir(&index_path) {
            Ok(e) => e,
            Err(_) => continue,
        };

        for crate_entry in crates.flatten() {
            let crate_path = crate_entry.path();
            let crate_name = crate_entry.file_name().to_string_lossy().to_string();

            if !crate_path.is_dir() {
                continue;
            }

            // Extract crate name (remove version suffix: "foo-1.2.3" -> "foo")
            let name = crate_name
                .rsplit_once('-')
                .map(|(n, _)| n)
                .unwrap_or(&crate_name);

            // Find src/lib.rs
            let lib_rs = crate_path.join("src").join("lib.rs");
            if lib_rs.is_file() {
                packages.push((name.to_string(), lib_rs));
            }
        }
    }

    packages
}

/// Find the crate root (directory containing Cargo.toml).
fn find_crate_root(start: &Path, root: &Path) -> Option<PathBuf> {
    let mut current = start.parent()?;
    while current.starts_with(root) {
        if current.join("Cargo.toml").exists() {
            return Some(current.to_path_buf());
        }
        current = current.parent()?;
    }
    None
}

// ============================================================================
// Rust local dependency discovery
// ============================================================================

/// Rust local dependency discovery.
pub struct RustDeps;

impl LocalDeps for RustDeps {
    fn ecosystem_key(&self) -> &'static str {
        "rust"
    }

    fn language_name(&self) -> &'static str {
        "Rust"
    }

    fn indexable_extensions(&self) -> &'static [&'static str] {
        &["rs"]
    }

    fn resolve_local_import(
        &self,
        module: &str,
        current_file: &Path,
        project_root: &Path,
    ) -> Option<PathBuf> {
        // Find the crate root (directory containing Cargo.toml)
        let crate_root = find_crate_root(current_file, project_root)?;

        if module.starts_with("crate::") {
            // crate::foo::bar -> src/foo/bar.rs or src/foo/bar/mod.rs
            let path_part = module.strip_prefix("crate::")?.replace("::", "/");
            let src_dir = crate_root.join("src");

            // Try foo/bar.rs
            let direct = src_dir.join(format!("{}.rs", path_part));
            if direct.exists() {
                return Some(direct);
            }

            // Try foo/bar/mod.rs
            let mod_file = src_dir.join(&path_part).join("mod.rs");
            if mod_file.exists() {
                return Some(mod_file);
            }
        } else if module.starts_with("super::") {
            // super::foo -> parent directory's foo
            let current_dir = current_file.parent()?;
            let parent_dir = current_dir.parent()?;
            let path_part = module.strip_prefix("super::")?.replace("::", "/");

            // Try parent/foo.rs
            let direct = parent_dir.join(format!("{}.rs", path_part));
            if direct.exists() {
                return Some(direct);
            }

            // Try parent/foo/mod.rs
            let mod_file = parent_dir.join(&path_part).join("mod.rs");
            if mod_file.exists() {
                return Some(mod_file);
            }
        } else if module.starts_with("self::") {
            // self::foo -> same directory's foo
            let current_dir = current_file.parent()?;
            let path_part = module.strip_prefix("self::")?.replace("::", "/");

            // Try dir/foo.rs
            let direct = current_dir.join(format!("{}.rs", path_part));
            if direct.exists() {
                return Some(direct);
            }

            // Try dir/foo/mod.rs
            let mod_file = current_dir.join(&path_part).join("mod.rs");
            if mod_file.exists() {
                return Some(mod_file);
            }
        }

        None
    }

    fn resolve_external_import(
        &self,
        crate_name: &str,
        _project_root: &Path,
    ) -> Option<ResolvedPackage> {
        let registry = find_cargo_registry()?;
        resolve_rust_crate(crate_name, &registry)
    }

    fn get_version(&self, _project_root: &Path) -> Option<String> {
        get_rust_version()
    }

    fn find_package_cache(&self, _project_root: &Path) -> Option<PathBuf> {
        find_cargo_registry()
    }

    fn is_stdlib_import(&self, _import_name: &str, _project_root: &Path) -> bool {
        // Rust stdlib is part of the compiler, no separate source to index
        false
    }

    fn find_stdlib(&self, _project_root: &Path) -> Option<PathBuf> {
        // Rust stdlib is part of the compiler, no separate path
        None
    }

    fn dep_sources(&self, project_root: &Path) -> Vec<LocalDepSource> {
        let mut sources = Vec::new();
        if let Some(cache) = self.find_package_cache(project_root) {
            sources.push(LocalDepSource {
                name: "cargo-registry",
                path: cache,
                kind: LocalDepSourceKind::Cargo,
                version_specific: false,
            });
        }
        sources
    }

    fn should_skip_dep_entry(&self, name: &str, is_dir: bool) -> bool {
        if skip_dotfiles(name) {
            return true;
        }
        // Skip target, tests directories
        if is_dir
            && (name == "target" || name == "tests" || name == "benches" || name == "examples")
        {
            return true;
        }
        // Only index .rs files
        !is_dir && !has_extension(name, self.indexable_extensions())
    }

    fn discover_packages(&self, source: &LocalDepSource) -> Vec<(String, PathBuf)> {
        if source.kind != LocalDepSourceKind::Cargo {
            return Vec::new();
        }
        discover_cargo_packages(&source.path)
    }

    fn dep_module_name(&self, entry_name: &str) -> String {
        // Strip .rs extension
        entry_name
            .strip_suffix(".rs")
            .unwrap_or(entry_name)
            .to_string()
    }

    fn find_package_entry(&self, path: &Path) -> Option<PathBuf> {
        if path.is_file() {
            return Some(path.to_path_buf());
        }
        // Rust packages use src/lib.rs as entry point
        let lib_rs = path.join("src").join("lib.rs");
        if lib_rs.is_file() {
            return Some(lib_rs);
        }
        // Or mod.rs in the directory itself
        let mod_rs = path.join("mod.rs");
        if mod_rs.is_file() {
            return Some(mod_rs);
        }
        None
    }

    fn file_path_to_module_name(&self, path: &Path) -> Option<String> {
        // Only Rust files
        if path.extension()?.to_str()? != "rs" {
            return None;
        }

        let path_str = path.to_str()?;

        // Strip src/ prefix if present
        let rel_path = path_str.strip_prefix("src/").unwrap_or(path_str);

        // Remove .rs extension
        let module_path = rel_path.strip_suffix(".rs")?;

        // Handle mod.rs and lib.rs - use parent directory as module
        let module_path = if module_path.ends_with("/mod") || module_path.ends_with("/lib") {
            module_path.rsplit_once('/')?.0
        } else {
            module_path
        };

        // Convert path separators to ::
        Some(module_path.replace('/', "::"))
    }

    fn module_name_to_paths(&self, module: &str) -> Vec<String> {
        let rel_path = module.replace("::", "/");

        vec![
            format!("src/{}.rs", rel_path),
            format!("src/{}/mod.rs", rel_path),
        ]
    }
}
