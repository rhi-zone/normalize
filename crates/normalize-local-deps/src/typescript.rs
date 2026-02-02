//! TypeScript local dependency resolution.

use crate::ResolvedPackage;
use crate::ecmascript;
use crate::{LocalDepSource, LocalDepSourceKind, LocalDeps};
use crate::{has_extension, skip_dotfiles};
use std::path::{Path, PathBuf};

/// TypeScript local dependency resolution.
pub struct TypeScriptDeps;

impl LocalDeps for TypeScriptDeps {
    fn ecosystem_key(&self) -> &'static str {
        "js"
    } // Uses same cache as JS

    fn language_name(&self) -> &'static str {
        "TypeScript"
    }

    fn resolve_local_import(
        &self,
        module: &str,
        current_file: &Path,
        _project_root: &Path,
    ) -> Option<PathBuf> {
        ecmascript::resolve_local_import(module, current_file, ecmascript::TS_EXTENSIONS)
    }

    fn resolve_external_import(
        &self,
        import_name: &str,
        project_root: &Path,
    ) -> Option<ResolvedPackage> {
        ecmascript::resolve_external_import(import_name, project_root)
    }

    fn get_version(&self, _project_root: &Path) -> Option<String> {
        ecmascript::get_version()
    }

    fn find_package_cache(&self, project_root: &Path) -> Option<PathBuf> {
        ecmascript::find_package_cache(project_root)
    }

    fn indexable_extensions(&self) -> &'static [&'static str] {
        &["ts", "mts", "cts", "js", "mjs", "cjs"]
    }

    fn dep_sources(&self, project_root: &Path) -> Vec<LocalDepSource> {
        let mut sources = Vec::new();
        if let Some(cache) = self.find_package_cache(project_root) {
            sources.push(LocalDepSource {
                name: "node_modules",
                path: cache,
                kind: LocalDepSourceKind::NpmScoped,
                version_specific: false,
            });
        }
        sources
    }

    fn should_skip_dep_entry(&self, name: &str, is_dir: bool) -> bool {
        if skip_dotfiles(name) {
            return true;
        }
        if is_dir && (name == "node_modules" || name == ".bin" || name == "test" || name == "tests")
        {
            return true;
        }
        !is_dir && !has_extension(name, self.indexable_extensions())
    }

    fn dep_module_name(&self, entry_name: &str) -> String {
        for ext in &[".ts", ".mts", ".cts", ".d.ts", ".js", ".mjs", ".cjs"] {
            if let Some(name) = entry_name.strip_suffix(ext) {
                return name.to_string();
            }
        }
        entry_name.to_string()
    }

    fn discover_packages(&self, source: &LocalDepSource) -> Vec<(String, PathBuf)> {
        self.discover_npm_scoped_packages(&source.path)
    }

    fn find_package_entry(&self, path: &Path) -> Option<PathBuf> {
        if path.is_file() {
            return Some(path.to_path_buf());
        }
        ecmascript::find_package_entry(path)
    }

    fn file_path_to_module_name(&self, path: &Path) -> Option<String> {
        let ext = path.extension()?.to_str()?;
        if !["ts", "mts", "cts", "tsx"].contains(&ext) {
            return None;
        }
        let stem = path.with_extension("");
        Some(stem.to_string_lossy().to_string())
    }

    fn module_name_to_paths(&self, module: &str) -> Vec<String> {
        vec![
            format!("{}.ts", module),
            format!("{}.tsx", module),
            format!("{}/index.ts", module),
        ]
    }

    fn is_stdlib_import(&self, _import_name: &str, _project_root: &Path) -> bool {
        false
    }

    fn find_stdlib(&self, _project_root: &Path) -> Option<PathBuf> {
        None
    }
}

/// TSX local dependency resolution (TypeScript + JSX).
pub struct TsxDeps;

impl LocalDeps for TsxDeps {
    fn ecosystem_key(&self) -> &'static str {
        "js"
    }

    fn language_name(&self) -> &'static str {
        "TSX"
    }

    fn resolve_local_import(
        &self,
        module: &str,
        current_file: &Path,
        _project_root: &Path,
    ) -> Option<PathBuf> {
        ecmascript::resolve_local_import(module, current_file, ecmascript::TS_EXTENSIONS)
    }

    fn resolve_external_import(
        &self,
        import_name: &str,
        project_root: &Path,
    ) -> Option<ResolvedPackage> {
        ecmascript::resolve_external_import(import_name, project_root)
    }

    fn get_version(&self, _project_root: &Path) -> Option<String> {
        ecmascript::get_version()
    }

    fn find_package_cache(&self, project_root: &Path) -> Option<PathBuf> {
        ecmascript::find_package_cache(project_root)
    }

    fn indexable_extensions(&self) -> &'static [&'static str] {
        &["tsx", "ts", "js"]
    }

    fn dep_sources(&self, project_root: &Path) -> Vec<LocalDepSource> {
        let mut sources = Vec::new();
        if let Some(cache) = self.find_package_cache(project_root) {
            sources.push(LocalDepSource {
                name: "node_modules",
                path: cache,
                kind: LocalDepSourceKind::NpmScoped,
                version_specific: false,
            });
        }
        sources
    }

    fn should_skip_dep_entry(&self, name: &str, is_dir: bool) -> bool {
        if skip_dotfiles(name) {
            return true;
        }
        if is_dir && (name == "node_modules" || name == ".bin" || name == "test" || name == "tests")
        {
            return true;
        }
        !is_dir && !has_extension(name, self.indexable_extensions())
    }

    fn dep_module_name(&self, entry_name: &str) -> String {
        for ext in &[".tsx", ".ts", ".d.ts", ".js"] {
            if let Some(name) = entry_name.strip_suffix(ext) {
                return name.to_string();
            }
        }
        entry_name.to_string()
    }

    fn discover_packages(&self, source: &LocalDepSource) -> Vec<(String, PathBuf)> {
        self.discover_npm_scoped_packages(&source.path)
    }

    fn find_package_entry(&self, path: &Path) -> Option<PathBuf> {
        if path.is_file() {
            return Some(path.to_path_buf());
        }
        ecmascript::find_package_entry(path)
    }

    fn file_path_to_module_name(&self, path: &Path) -> Option<String> {
        let ext = path.extension()?.to_str()?;
        if ext != "tsx" {
            return None;
        }
        let stem = path.with_extension("");
        Some(stem.to_string_lossy().to_string())
    }

    fn module_name_to_paths(&self, module: &str) -> Vec<String> {
        vec![format!("{}.tsx", module), format!("{}/index.tsx", module)]
    }

    fn is_stdlib_import(&self, _import_name: &str, _project_root: &Path) -> bool {
        false
    }

    fn find_stdlib(&self, _project_root: &Path) -> Option<PathBuf> {
        None
    }
}
