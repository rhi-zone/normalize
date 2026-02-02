//! C local dependency resolution.

use crate::ResolvedPackage;
use crate::c_cpp;
use crate::{LocalDepSource, LocalDepSourceKind, LocalDeps};
use crate::{has_extension, skip_dotfiles};
use std::path::{Path, PathBuf};

/// C local dependency resolution.
pub struct CDeps;

impl LocalDeps for CDeps {
    fn ecosystem_key(&self) -> &'static str {
        "c"
    }

    fn language_name(&self) -> &'static str {
        "C"
    }

    fn resolve_local_import(
        &self,
        include: &str,
        current_file: &Path,
        _project_root: &Path,
    ) -> Option<PathBuf> {
        // Strip quotes if present
        let header = include
            .trim_start_matches('"')
            .trim_end_matches('"')
            .trim_start_matches('<')
            .trim_end_matches('>');

        let current_dir = current_file.parent()?;

        // Try relative to current file's directory
        let relative = current_dir.join(header);
        if relative.is_file() {
            return Some(relative);
        }

        // Try with common extensions if none specified
        if !header.contains('.') {
            for ext in &[".h", ".c"] {
                let with_ext = current_dir.join(format!("{}{}", header, ext));
                if with_ext.is_file() {
                    return Some(with_ext);
                }
            }
        }

        None
    }

    fn resolve_external_import(
        &self,
        include: &str,
        _project_root: &Path,
    ) -> Option<ResolvedPackage> {
        let include_paths = c_cpp::find_cpp_include_paths();
        c_cpp::resolve_cpp_include(include, &include_paths)
    }

    fn get_version(&self, _project_root: &Path) -> Option<String> {
        c_cpp::get_gcc_version()
    }

    fn find_package_cache(&self, _project_root: &Path) -> Option<PathBuf> {
        None // C uses include paths, not a package cache
    }

    fn find_stdlib(&self, _project_root: &Path) -> Option<PathBuf> {
        // Return the first include path as stdlib location
        c_cpp::find_cpp_include_paths().into_iter().next()
    }

    fn indexable_extensions(&self) -> &'static [&'static str] {
        &["c", "h"]
    }

    fn dep_sources(&self, _project_root: &Path) -> Vec<LocalDepSource> {
        c_cpp::find_cpp_include_paths()
            .into_iter()
            .map(|path| LocalDepSource {
                name: "includes",
                path,
                kind: LocalDepSourceKind::Recursive,
                version_specific: false,
            })
            .collect()
    }

    fn dep_module_name(&self, entry_name: &str) -> String {
        entry_name.to_string()
    }

    fn discover_packages(&self, source: &LocalDepSource) -> Vec<(String, PathBuf)> {
        self.discover_recursive_packages(&source.path, &source.path)
    }

    fn find_package_entry(&self, path: &Path) -> Option<PathBuf> {
        if path.is_file() {
            Some(path.to_path_buf())
        } else {
            None
        }
    }

    fn should_skip_dep_entry(&self, name: &str, is_dir: bool) -> bool {
        if skip_dotfiles(name) {
            return true;
        }
        !is_dir && !has_extension(name, self.indexable_extensions())
    }

    fn is_stdlib_import(&self, include: &str, _project_root: &Path) -> bool {
        // Standard C headers
        let stdlib = [
            "stdio.h", "stdlib.h", "string.h", "math.h", "time.h", "ctype.h", "errno.h", "float.h",
            "limits.h", "locale.h", "setjmp.h", "signal.h", "stdarg.h", "stddef.h", "assert.h",
        ];
        stdlib.contains(&include)
    }

    fn file_path_to_module_name(&self, path: &Path) -> Option<String> {
        let ext = path.extension()?.to_str()?;
        if !["c", "h"].contains(&ext) {
            return None;
        }
        Some(path.to_string_lossy().to_string())
    }

    fn module_name_to_paths(&self, module: &str) -> Vec<String> {
        vec![module.to_string()]
    }
}
