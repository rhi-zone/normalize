//! C++ local dependency resolution.

use crate::ResolvedPackage;
use crate::c_cpp;
use crate::skip_dotfiles;
use crate::{LocalDepSource, LocalDepSourceKind, LocalDeps};
use std::path::{Path, PathBuf};

/// C++ local dependency resolution.
pub struct CppDeps;

impl LocalDeps for CppDeps {
    fn ecosystem_key(&self) -> &'static str {
        "cpp"
    }

    fn language_name(&self) -> &'static str {
        "C++"
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
            for ext in &[".h", ".hpp", ".hxx", ".hh"] {
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
        None
    }

    fn find_stdlib(&self, _project_root: &Path) -> Option<PathBuf> {
        c_cpp::find_cpp_include_paths().into_iter().next()
    }

    fn indexable_extensions(&self) -> &'static [&'static str] {
        &["cpp", "hpp", "cc", "hh", "cxx", "hxx", "h"]
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

    fn should_skip_dep_entry(&self, name: &str, is_dir: bool) -> bool {
        if skip_dotfiles(name) {
            return true;
        }
        // Skip the "bits" directory (C++ internal headers)
        if is_dir && name == "bits" {
            return true;
        }
        if is_dir {
            return false;
        }
        // Check if it's a valid header: explicit extensions or extensionless stdlib headers
        let is_header = name.ends_with(".h")
            || name.ends_with(".hpp")
            || name.ends_with(".hxx")
            || name.ends_with(".hh")
            // C++ standard library headers (no extension, like vector, iostream)
            || (!name.contains('.') && !name.contains('-')
                && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'));
        !is_header
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

    fn is_stdlib_import(&self, include: &str, _project_root: &Path) -> bool {
        // C++ standard library headers (no extension)
        let stdlib = [
            "iostream",
            "vector",
            "string",
            "map",
            "set",
            "algorithm",
            "memory",
            "utility",
            "functional",
            "iterator",
            "numeric",
            "cstdio",
            "cstdlib",
            "cstring",
            "cmath",
            "climits",
        ];
        stdlib.contains(&include)
    }

    fn file_path_to_module_name(&self, path: &Path) -> Option<String> {
        let ext = path.extension()?.to_str()?;
        if !["cpp", "cc", "cxx", "hpp", "hh", "hxx", "h"].contains(&ext) {
            return None;
        }
        Some(path.to_string_lossy().to_string())
    }

    fn module_name_to_paths(&self, module: &str) -> Vec<String> {
        vec![module.to_string()]
    }
}
