//! Python local dependency discovery.

use crate::{LocalDepSource, LocalDepSourceKind, LocalDeps, ResolvedPackage};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

// ============================================================================
// Python path cache (filesystem-based detection, no subprocess calls)
// ============================================================================

static PYTHON_CACHE: Mutex<Option<PythonPathCache>> = Mutex::new(None);

/// Cached Python paths detected from filesystem structure.
#[derive(Clone)]
struct PythonPathCache {
    /// Canonical project root used as cache key
    root: PathBuf,
    /// Python version (e.g., "3.13")
    version: Option<String>,
    /// Stdlib path (e.g., /usr/.../lib/python3.13/)
    stdlib: Option<PathBuf>,
    /// Site-packages path
    site_packages: Option<PathBuf>,
}

impl PythonPathCache {
    fn new(root: &Path) -> Self {
        let root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());

        // Try to find Python from venv or PATH
        let python_bin = if root.join(".venv/bin/python").exists() {
            Some(root.join(".venv/bin/python"))
        } else if root.join("venv/bin/python").exists() {
            Some(root.join("venv/bin/python"))
        } else {
            // Look in PATH
            std::env::var("PATH").ok().and_then(|path| {
                for dir in path.split(':') {
                    let python = PathBuf::from(dir).join("python3");
                    if python.exists() {
                        return Some(python);
                    }
                    let python = PathBuf::from(dir).join("python");
                    if python.exists() {
                        return Some(python);
                    }
                }
                None
            })
        };

        let Some(python_bin) = python_bin else {
            return Self {
                root,
                version: None,
                stdlib: None,
                site_packages: None,
            };
        };

        // Resolve symlinks to find the actual Python installation
        let python_real = std::fs::canonicalize(&python_bin).unwrap_or(python_bin.clone());

        // Python binary is typically at /prefix/bin/python3
        // Stdlib is at /prefix/lib/pythonX.Y/
        // Site-packages is at /prefix/lib/pythonX.Y/site-packages/ (system)
        // Or for venv: venv/lib/pythonX.Y/site-packages/

        let prefix = python_real.parent().and_then(|bin| bin.parent());

        // Look for lib/pythonX.Y directories to detect version
        let (version, stdlib, site_packages) = if let Some(prefix) = prefix {
            let lib = prefix.join("lib");
            if lib.exists() {
                // Find pythonX.Y directories
                let mut best_version: Option<(String, PathBuf)> = None;
                if let Ok(entries) = std::fs::read_dir(&lib) {
                    for entry in entries.flatten() {
                        let name = entry.file_name();
                        let name = name.to_string_lossy();
                        if name.starts_with("python") && entry.path().is_dir() {
                            let ver = name.trim_start_matches("python");
                            // Check it looks like a version (X.Y)
                            if ver.contains('.')
                                && ver.chars().next().is_some_and(|c| c.is_ascii_digit())
                            {
                                // Prefer higher versions
                                if best_version.as_ref().is_none_or(|(v, _)| ver > v.as_str()) {
                                    best_version = Some((ver.to_string(), entry.path()));
                                }
                            }
                        }
                    }
                }

                if let Some((ver, stdlib_path)) = best_version {
                    // For venv, site-packages is in the venv
                    let site = if root.join(".venv").exists() || root.join("venv").exists() {
                        let venv = if root.join(".venv").exists() {
                            root.join(".venv")
                        } else {
                            root.join("venv")
                        };
                        let venv_site = venv
                            .join("lib")
                            .join(format!("python{}", ver))
                            .join("site-packages");
                        if venv_site.exists() {
                            Some(venv_site)
                        } else {
                            // Fall back to system site-packages
                            let sys_site = stdlib_path.join("site-packages");
                            if sys_site.exists() {
                                Some(sys_site)
                            } else {
                                None
                            }
                        }
                    } else {
                        let sys_site = stdlib_path.join("site-packages");
                        if sys_site.exists() {
                            Some(sys_site)
                        } else {
                            None
                        }
                    };

                    (Some(ver), Some(stdlib_path), site)
                } else {
                    (None, None, None)
                }
            } else {
                (None, None, None)
            }
        } else {
            (None, None, None)
        };

        Self {
            root,
            version,
            stdlib,
            site_packages,
        }
    }
}

/// Get cached Python paths for a project.
fn get_python_cache(project_root: &Path) -> PythonPathCache {
    let canonical = project_root
        .canonicalize()
        .unwrap_or_else(|_| project_root.to_path_buf());

    let mut cache_guard = PYTHON_CACHE.lock().unwrap();

    if let Some(ref cache) = *cache_guard
        && cache.root == canonical
    {
        return cache.clone();
    }

    let new_cache = PythonPathCache::new(project_root);
    *cache_guard = Some(new_cache.clone());
    new_cache
}

// ============================================================================
// Python stdlib and site-packages resolution
// ============================================================================

/// Get Python version from filesystem structure (no subprocess).
pub fn get_python_version(project_root: &Path) -> Option<String> {
    get_python_cache(project_root).version
}

/// Find Python stdlib directory from filesystem structure (no subprocess).
pub fn find_python_stdlib(project_root: &Path) -> Option<PathBuf> {
    get_python_cache(project_root).stdlib
}

/// Check if a module name is a Python stdlib module.
fn is_python_stdlib_module(module_name: &str, stdlib_path: &Path) -> bool {
    let top_level = module_name.split('.').next().unwrap_or(module_name);

    // Check for package
    let pkg_dir = stdlib_path.join(top_level);
    if pkg_dir.is_dir() {
        return true;
    }

    // Check for module
    let py_file = stdlib_path.join(format!("{}.py", top_level));
    py_file.is_file()
}

/// Resolve a Python stdlib import to its source location.
fn resolve_python_stdlib_import(import_name: &str, stdlib_path: &Path) -> Option<ResolvedPackage> {
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
    // Use cached result from filesystem detection
    if let Some(site) = get_python_cache(project_root).site_packages {
        return Some(site);
    }

    // Fall back to scanning parent directories for venvs
    let mut current = project_root.to_path_buf();
    while let Some(parent) = current.parent() {
        let venv_dir = parent.join(".venv");
        if venv_dir.is_dir()
            && let Some(site_packages) = find_site_packages_in_venv(&venv_dir)
        {
            return Some(site_packages);
        }
        current = parent.to_path_buf();
    }

    None
}

/// Find site-packages within a venv directory.
fn find_site_packages_in_venv(venv: &Path) -> Option<PathBuf> {
    // Unix: lib/pythonX.Y/site-packages
    let lib_dir = venv.join("lib");
    if lib_dir.is_dir()
        && let Ok(entries) = std::fs::read_dir(&lib_dir)
    {
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
fn resolve_python_import(import_name: &str, site_packages: &Path) -> Option<ResolvedPackage> {
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

// ============================================================================
// Python local dependency discovery
// ============================================================================

/// Python local dependency discovery.
pub struct PythonDeps;

impl LocalDeps for PythonDeps {
    fn ecosystem_key(&self) -> &'static str {
        "python"
    }

    fn language_name(&self) -> &'static str {
        "Python"
    }

    fn indexable_extensions(&self) -> &'static [&'static str] {
        &["py"]
    }

    fn resolve_local_import(
        &self,
        import_name: &str,
        current_file: &Path,
        project_root: &Path,
    ) -> Option<PathBuf> {
        // Handle relative imports (starting with .)
        if import_name.starts_with('.') {
            let current_dir = current_file.parent()?;
            let dots = import_name.chars().take_while(|c| *c == '.').count();
            let module_part = &import_name[dots..];

            // Go up (dots-1) directories from current file's directory
            let mut base = current_dir.to_path_buf();
            for _ in 1..dots {
                base = base.parent()?.to_path_buf();
            }

            // Convert module.path to module/path.py
            let module_path = if module_part.is_empty() {
                base.join("__init__.py")
            } else {
                let path_part = module_part.replace('.', "/");
                // Try module/submodule.py first, then module/submodule/__init__.py
                let direct = base.join(format!("{}.py", path_part));
                if direct.exists() {
                    return Some(direct);
                }
                base.join(path_part).join("__init__.py")
            };

            if module_path.exists() {
                return Some(module_path);
            }
        }

        // Handle absolute imports - try to find in src/ or as top-level package
        let module_path = import_name.replace('.', "/");

        // Try src/<module>.py
        let src_path = project_root.join("src").join(format!("{}.py", module_path));
        if src_path.exists() {
            return Some(src_path);
        }

        // Try src/<module>/__init__.py
        let src_pkg_path = project_root
            .join("src")
            .join(&module_path)
            .join("__init__.py");
        if src_pkg_path.exists() {
            return Some(src_pkg_path);
        }

        // Try <module>.py directly
        let direct_path = project_root.join(format!("{}.py", module_path));
        if direct_path.exists() {
            return Some(direct_path);
        }

        // Try <module>/__init__.py
        let pkg_path = project_root.join(&module_path).join("__init__.py");
        if pkg_path.exists() {
            return Some(pkg_path);
        }

        None
    }

    fn resolve_external_import(
        &self,
        import_name: &str,
        project_root: &Path,
    ) -> Option<ResolvedPackage> {
        // Check stdlib first
        if let Some(stdlib) = find_python_stdlib(project_root)
            && let Some(pkg) = resolve_python_stdlib_import(import_name, &stdlib)
        {
            return Some(pkg);
        }

        // Then site-packages
        if let Some(site_packages) = find_python_site_packages(project_root) {
            return resolve_python_import(import_name, &site_packages);
        }

        None
    }

    fn is_stdlib_import(&self, import_name: &str, project_root: &Path) -> bool {
        if let Some(stdlib) = find_python_stdlib(project_root) {
            is_python_stdlib_module(import_name, &stdlib)
        } else {
            false
        }
    }

    fn get_version(&self, project_root: &Path) -> Option<String> {
        get_python_version(project_root)
    }

    fn find_package_cache(&self, project_root: &Path) -> Option<PathBuf> {
        find_python_site_packages(project_root)
    }

    fn find_stdlib(&self, project_root: &Path) -> Option<PathBuf> {
        find_python_stdlib(project_root)
    }

    fn should_skip_dep_entry(&self, name: &str, is_dir: bool) -> bool {
        // Skip private modules
        if name.starts_with('_') {
            return true;
        }
        // Skip __pycache__, dist-info, egg-info
        if name == "__pycache__" || name.ends_with(".dist-info") || name.ends_with(".egg-info") {
            return true;
        }
        // Skip non-Python files
        if !is_dir && !name.ends_with(".py") {
            return true;
        }
        false
    }

    fn find_package_entry(&self, path: &Path) -> Option<PathBuf> {
        if path.is_file() {
            return Some(path.to_path_buf());
        }
        // Python packages use __init__.py as entry point
        let init_py = path.join("__init__.py");
        if init_py.is_file() {
            return Some(init_py);
        }
        None
    }

    fn dep_module_name(&self, entry_name: &str) -> String {
        // Strip .py extension
        entry_name
            .strip_suffix(".py")
            .unwrap_or(entry_name)
            .to_string()
    }

    fn dep_sources(&self, project_root: &Path) -> Vec<LocalDepSource> {
        let mut sources = Vec::new();
        if let Some(stdlib) = self.find_stdlib(project_root) {
            sources.push(LocalDepSource {
                name: "stdlib",
                path: stdlib,
                kind: LocalDepSourceKind::Flat,
                version_specific: true,
            });
        }
        if let Some(cache) = self.find_package_cache(project_root) {
            sources.push(LocalDepSource {
                name: "site-packages",
                path: cache,
                kind: LocalDepSourceKind::Flat,
                version_specific: false,
            });
        }
        sources
    }

    fn discover_packages(&self, source: &LocalDepSource) -> Vec<(String, PathBuf)> {
        self.discover_flat_packages(&source.path)
    }

    fn file_path_to_module_name(&self, path: &Path) -> Option<String> {
        // Only Python files
        if path.extension()?.to_str()? != "py" {
            return None;
        }

        // Remove extension
        let stem = path.with_extension("");
        let stem_str = stem.to_str()?;

        // Strip common source directory prefixes
        let module_path = stem_str
            .strip_prefix("src/")
            .or_else(|| stem_str.strip_prefix("lib/"))
            .unwrap_or(stem_str);

        // Handle __init__.py - use parent directory as module
        let module_path = if module_path.ends_with("/__init__") {
            module_path.strip_suffix("/__init__")?
        } else {
            module_path
        };

        // Convert path separators to dots
        Some(module_path.replace('/', "."))
    }

    fn module_name_to_paths(&self, module: &str) -> Vec<String> {
        // Convert dots to path separators
        let rel_path = module.replace('.', "/");

        // Try common source directories and both .py and __init__.py
        let mut candidates = Vec::with_capacity(4);
        for prefix in &["src/", ""] {
            candidates.push(format!("{}{}.py", prefix, rel_path));
            candidates.push(format!("{}{}/__init__.py", prefix, rel_path));
        }
        candidates
    }
}
