//! External package resolution for Python and Go.
//!
//! Finds installed packages, stdlib, and resolves import paths to their source files.
//! Uses a global cache at ~/.cache/moss/ for indexed packages.

use std::path::{Path, PathBuf};
use std::process::Command;

// =============================================================================
// Global Cache
// =============================================================================

/// Get the global moss cache directory (~/.cache/moss/).
pub fn get_global_cache_dir() -> Option<PathBuf> {
    // XDG_CACHE_HOME or ~/.cache
    let cache_base = if let Ok(xdg) = std::env::var("XDG_CACHE_HOME") {
        PathBuf::from(xdg)
    } else if let Ok(home) = std::env::var("HOME") {
        PathBuf::from(home).join(".cache")
    } else if let Ok(home) = std::env::var("USERPROFILE") {
        // Windows
        PathBuf::from(home).join(".cache")
    } else {
        return None;
    };

    let moss_cache = cache_base.join("moss");

    // Create if doesn't exist
    if !moss_cache.exists() {
        std::fs::create_dir_all(&moss_cache).ok()?;
    }

    Some(moss_cache)
}

/// Get the path to the unified global package index database.
/// e.g., ~/.cache/moss/packages.db
///
/// Schema:
/// - packages(id, language, name, path, min_major, min_minor, max_major, max_minor, indexed_at)
/// - symbols(id, package_id, name, kind, signature, line)
///
/// Version stored as (major, minor) integers for proper comparison.
/// max_major/max_minor NULL means "any version".
pub fn get_global_packages_db() -> Option<PathBuf> {
    let cache = get_global_cache_dir()?;
    Some(cache.join("packages.db"))
}

/// Get Python version from the project's interpreter.
pub fn get_python_version(project_root: &Path) -> Option<String> {
    let python = if project_root.join(".venv/bin/python").exists() {
        project_root.join(".venv/bin/python")
    } else if project_root.join("venv/bin/python").exists() {
        project_root.join("venv/bin/python")
    } else {
        PathBuf::from("python3")
    };

    let output = Command::new(&python)
        .args(["-c", "import sys; print(f'{sys.version_info.major}.{sys.version_info.minor}')"])
        .output()
        .ok()?;

    if output.status.success() {
        Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        None
    }
}

/// Get Go version.
pub fn get_go_version() -> Option<String> {
    let output = Command::new("go").args(["version"]).output().ok()?;

    if output.status.success() {
        let version_str = String::from_utf8_lossy(&output.stdout);
        // "go version go1.21.0 linux/amd64" -> "1.21"
        for part in version_str.split_whitespace() {
            if part.starts_with("go") && part.len() > 2 {
                let ver = part.trim_start_matches("go");
                // Take major.minor only
                let parts: Vec<&str> = ver.split('.').collect();
                if parts.len() >= 2 {
                    return Some(format!("{}.{}", parts[0], parts[1]));
                }
            }
        }
    }

    None
}

/// Result of resolving an external package
#[derive(Debug, Clone)]
pub struct ResolvedPackage {
    /// Path to the package source
    pub path: PathBuf,
    /// Package name as imported
    pub name: String,
    /// Whether this is a namespace package (no __init__.py)
    pub is_namespace: bool,
}

// =============================================================================
// Python
// =============================================================================

/// Find Python stdlib directory.
///
/// Uses `python -c "import sys; print(sys.prefix)"` to find the prefix,
/// then looks for lib/pythonX.Y/ underneath.
pub fn find_python_stdlib(project_root: &Path) -> Option<PathBuf> {
    // Try to use the project's Python first (from venv)
    let python = if project_root.join(".venv/bin/python").exists() {
        project_root.join(".venv/bin/python")
    } else if project_root.join("venv/bin/python").exists() {
        project_root.join("venv/bin/python")
    } else {
        PathBuf::from("python3")
    };

    // Get sys.prefix and sys.version_info
    let output = Command::new(&python)
        .args(["-c", "import sys; print(sys.prefix); print(f'{sys.version_info.major}.{sys.version_info.minor}')"])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut lines = stdout.lines();
    let prefix = lines.next()?.trim();
    let version = lines.next()?.trim();

    // Unix: lib/pythonX.Y
    let stdlib = PathBuf::from(prefix).join("lib").join(format!("python{}", version));
    if stdlib.is_dir() {
        return Some(stdlib);
    }

    // Windows: Lib
    let stdlib = PathBuf::from(prefix).join("Lib");
    if stdlib.is_dir() {
        return Some(stdlib);
    }

    None
}

/// Check if a module name is a Python stdlib module.
pub fn is_python_stdlib_module(module_name: &str, stdlib_path: &Path) -> bool {
    let top_level = module_name.split('.').next().unwrap_or(module_name);

    // Check for package
    let pkg_dir = stdlib_path.join(top_level);
    if pkg_dir.is_dir() {
        return true;
    }

    // Check for module
    let py_file = stdlib_path.join(format!("{}.py", top_level));
    if py_file.is_file() {
        return true;
    }

    false
}

/// Resolve a Python stdlib import to its source location.
pub fn resolve_python_stdlib_import(import_name: &str, stdlib_path: &Path) -> Option<ResolvedPackage> {
    let parts: Vec<&str> = import_name.split('.').collect();
    let top_level = parts[0];

    // Check for package (directory)
    let pkg_dir = stdlib_path.join(top_level);
    if pkg_dir.is_dir() {
        if parts.len() == 1 {
            let init = pkg_dir.join("__init__.py");
            if init.is_file() {
                return Some(ResolvedPackage {
                    path: pkg_dir,
                    name: import_name.to_string(),
                    is_namespace: false,
                });
            }
            // Some stdlib packages don't have __init__.py in newer Python
            return Some(ResolvedPackage {
                path: pkg_dir,
                name: import_name.to_string(),
                is_namespace: true,
            });
        } else {
            // Submodule
            let mut path = pkg_dir.clone();
            for part in &parts[1..] {
                path = path.join(part);
            }

            if path.is_dir() {
                let init = path.join("__init__.py");
                return Some(ResolvedPackage {
                    path: path.clone(),
                    name: import_name.to_string(),
                    is_namespace: !init.is_file(),
                });
            }

            let py_file = path.with_extension("py");
            if py_file.is_file() {
                return Some(ResolvedPackage {
                    path: py_file,
                    name: import_name.to_string(),
                    is_namespace: false,
                });
            }

            return None;
        }
    }

    // Check for single-file module
    let py_file = stdlib_path.join(format!("{}.py", top_level));
    if py_file.is_file() {
        return Some(ResolvedPackage {
            path: py_file,
            name: import_name.to_string(),
            is_namespace: false,
        });
    }

    None
}

/// Find Python site-packages directory for a project.
///
/// Search order:
/// 1. .venv/lib/pythonX.Y/site-packages/ (uv, poetry, standard venv)
/// 2. Walk up looking for venv directories
pub fn find_python_site_packages(project_root: &Path) -> Option<PathBuf> {
    // Check .venv in project root first (most common with uv/poetry)
    let venv_dir = project_root.join(".venv");
    if venv_dir.is_dir() {
        if let Some(site_packages) = find_site_packages_in_venv(&venv_dir) {
            return Some(site_packages);
        }
    }

    // Check venv (alternative name)
    let venv_dir = project_root.join("venv");
    if venv_dir.is_dir() {
        if let Some(site_packages) = find_site_packages_in_venv(&venv_dir) {
            return Some(site_packages);
        }
    }

    // Check .venv in parent directories
    let mut current = project_root.to_path_buf();
    while let Some(parent) = current.parent() {
        let venv_dir = parent.join(".venv");
        if venv_dir.is_dir() {
            if let Some(site_packages) = find_site_packages_in_venv(&venv_dir) {
                return Some(site_packages);
            }
        }
        current = parent.to_path_buf();
    }

    None
}

/// Find site-packages within a venv directory.
fn find_site_packages_in_venv(venv: &Path) -> Option<PathBuf> {
    // Unix: lib/pythonX.Y/site-packages
    let lib_dir = venv.join("lib");
    if lib_dir.is_dir() {
        if let Ok(entries) = std::fs::read_dir(&lib_dir) {
            for entry in entries.flatten() {
                let name = entry.file_name();
                let name_str = name.to_string_lossy();
                if name_str.starts_with("python") {
                    let site_packages = entry.path().join("site-packages");
                    if site_packages.is_dir() {
                        return Some(site_packages);
                    }
                }
            }
        }
    }

    // Windows: Lib/site-packages
    let lib_dir = venv.join("Lib").join("site-packages");
    if lib_dir.is_dir() {
        return Some(lib_dir);
    }

    None
}

/// Resolve a Python import to its source location.
///
/// Handles:
/// - Package imports (requests -> requests/__init__.py)
/// - Module imports (six -> six.py)
/// - Submodule imports (requests.api -> requests/api.py)
/// - Namespace packages (no __init__.py)
pub fn resolve_python_import(import_name: &str, site_packages: &Path) -> Option<ResolvedPackage> {
    // Split on dots for submodule resolution
    let parts: Vec<&str> = import_name.split('.').collect();
    let top_level = parts[0];

    // Check for package (directory)
    let pkg_dir = site_packages.join(top_level);
    if pkg_dir.is_dir() {
        if parts.len() == 1 {
            // Just the package - look for __init__.py
            let init = pkg_dir.join("__init__.py");
            if init.is_file() {
                return Some(ResolvedPackage {
                    path: pkg_dir,
                    name: import_name.to_string(),
                    is_namespace: false,
                });
            }
            // Namespace package (no __init__.py)
            return Some(ResolvedPackage {
                path: pkg_dir,
                name: import_name.to_string(),
                is_namespace: true,
            });
        } else {
            // Submodule - build path
            let mut path = pkg_dir.clone();
            for part in &parts[1..] {
                path = path.join(part);
            }

            // Try as package first
            if path.is_dir() {
                let init = path.join("__init__.py");
                return Some(ResolvedPackage {
                    path: path.clone(),
                    name: import_name.to_string(),
                    is_namespace: !init.is_file(),
                });
            }

            // Try as module
            let py_file = path.with_extension("py");
            if py_file.is_file() {
                return Some(ResolvedPackage {
                    path: py_file,
                    name: import_name.to_string(),
                    is_namespace: false,
                });
            }

            return None;
        }
    }

    // Check for single-file module
    let py_file = site_packages.join(format!("{}.py", top_level));
    if py_file.is_file() {
        return Some(ResolvedPackage {
            path: py_file,
            name: import_name.to_string(),
            is_namespace: false,
        });
    }

    None
}

// =============================================================================
// Go
// =============================================================================

/// Find Go stdlib directory (GOROOT/src).
pub fn find_go_stdlib() -> Option<PathBuf> {
    // Try GOROOT env var
    if let Ok(goroot) = std::env::var("GOROOT") {
        let src = PathBuf::from(goroot).join("src");
        if src.is_dir() {
            return Some(src);
        }
    }

    // Try `go env GOROOT`
    if let Ok(output) = Command::new("go").args(["env", "GOROOT"]).output() {
        if output.status.success() {
            let goroot = String::from_utf8_lossy(&output.stdout).trim().to_string();
            let src = PathBuf::from(goroot).join("src");
            if src.is_dir() {
                return Some(src);
            }
        }
    }

    // Common locations
    for path in &["/usr/local/go/src", "/usr/lib/go/src", "/opt/go/src"] {
        let src = PathBuf::from(path);
        if src.is_dir() {
            return Some(src);
        }
    }

    None
}

/// Check if a Go import is a stdlib import (no dots in first path segment).
pub fn is_go_stdlib_import(import_path: &str) -> bool {
    let first_segment = import_path.split('/').next().unwrap_or(import_path);
    !first_segment.contains('.')
}

/// Resolve a Go stdlib import to its source location.
pub fn resolve_go_stdlib_import(import_path: &str, stdlib_path: &Path) -> Option<ResolvedPackage> {
    if !is_go_stdlib_import(import_path) {
        return None;
    }

    let pkg_dir = stdlib_path.join(import_path);
    if pkg_dir.is_dir() {
        return Some(ResolvedPackage {
            path: pkg_dir,
            name: import_path.to_string(),
            is_namespace: false,
        });
    }

    None
}

/// Find Go module cache directory.
///
/// Uses GOMODCACHE env var, falls back to ~/go/pkg/mod
pub fn find_go_mod_cache() -> Option<PathBuf> {
    // Check GOMODCACHE env var
    if let Ok(cache) = std::env::var("GOMODCACHE") {
        let path = PathBuf::from(cache);
        if path.is_dir() {
            return Some(path);
        }
    }

    // Fall back to ~/go/pkg/mod using HOME env var
    if let Ok(home) = std::env::var("HOME") {
        let mod_cache = PathBuf::from(home).join("go").join("pkg").join("mod");
        if mod_cache.is_dir() {
            return Some(mod_cache);
        }
    }

    // Windows fallback
    if let Ok(home) = std::env::var("USERPROFILE") {
        let mod_cache = PathBuf::from(home).join("go").join("pkg").join("mod");
        if mod_cache.is_dir() {
            return Some(mod_cache);
        }
    }

    None
}

/// Resolve a Go import to its source location.
///
/// Import paths like "github.com/user/repo/pkg" are mapped to
/// $GOMODCACHE/github.com/user/repo@version/pkg
pub fn resolve_go_import(import_path: &str, mod_cache: &Path) -> Option<ResolvedPackage> {
    // Skip standard library imports (no dots in first segment)
    let first_segment = import_path.split('/').next()?;
    if !first_segment.contains('.') {
        // This is stdlib (fmt, os, etc.) - not in mod cache
        return None;
    }

    // Find the module in cache
    // Import path: github.com/user/repo/internal/pkg
    // Cache path: github.com/user/repo@v1.2.3/internal/pkg

    // We need to find the right version directory
    // Start with the full path and try progressively shorter prefixes
    let parts: Vec<&str> = import_path.split('/').collect();

    for i in (2..=parts.len()).rev() {
        let module_prefix = parts[..i].join("/");
        let module_dir = mod_cache.join(&module_prefix);

        // The parent directory might contain version directories
        if let Some(parent) = module_dir.parent() {
            if parent.is_dir() {
                // Look for versioned directories matching this module
                let module_name = module_dir.file_name()?.to_string_lossy();
                if let Ok(entries) = std::fs::read_dir(parent) {
                    for entry in entries.flatten() {
                        let name = entry.file_name();
                        let name_str = name.to_string_lossy();
                        // Match module@version pattern
                        if name_str.starts_with(&format!("{}@", module_name)) {
                            let versioned_path = entry.path();
                            // Add remaining path components
                            let remainder = if i < parts.len() {
                                parts[i..].join("/")
                            } else {
                                String::new()
                            };
                            let full_path = if remainder.is_empty() {
                                versioned_path.clone()
                            } else {
                                versioned_path.join(&remainder)
                            };

                            if full_path.is_dir() {
                                return Some(ResolvedPackage {
                                    path: full_path,
                                    name: import_path.to_string(),
                                    is_namespace: false,
                                });
                            }
                        }
                    }
                }
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_site_packages() {
        // Test with current project (has .venv)
        let root = std::env::current_dir().unwrap();
        let site_packages = find_python_site_packages(&root);
        // This test assumes we're running from moss project root with .venv
        if root.join(".venv").exists() {
            assert!(site_packages.is_some());
            let sp = site_packages.unwrap();
            assert!(sp.to_string_lossy().contains("site-packages"));
        }
    }

    #[test]
    fn test_resolve_python_import() {
        let root = std::env::current_dir().unwrap();
        if let Some(site_packages) = find_python_site_packages(&root) {
            // Try to resolve a common package
            if let Some(pkg) = resolve_python_import("pathlib", &site_packages) {
                // pathlib might be stdlib, skip
                let _ = pkg;
            }

            // Try requests if installed
            if let Some(pkg) = resolve_python_import("ruff", &site_packages) {
                assert!(pkg.path.exists());
                assert_eq!(pkg.name, "ruff");
            }
        }
    }
}
