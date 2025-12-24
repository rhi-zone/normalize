//! JavaScript language support.

use std::path::{Path, PathBuf};
use crate::{Export, Import, Language, Symbol, VisibilityMechanism};
use crate::ecmascript;
use crate::external_packages::ResolvedPackage;
use moss_core::tree_sitter::Node;

/// JavaScript language support.
pub struct JavaScript;

impl Language for JavaScript {
    fn name(&self) -> &'static str { "JavaScript" }
    fn extensions(&self) -> &'static [&'static str] { &["js", "mjs", "cjs", "jsx"] }
    fn grammar_name(&self) -> &'static str { "javascript" }

    fn container_kinds(&self) -> &'static [&'static str] { ecmascript::CONTAINER_KINDS }
    fn function_kinds(&self) -> &'static [&'static str] { ecmascript::JS_FUNCTION_KINDS }
    fn type_kinds(&self) -> &'static [&'static str] { ecmascript::JS_TYPE_KINDS }
    fn import_kinds(&self) -> &'static [&'static str] { ecmascript::IMPORT_KINDS }
    fn public_symbol_kinds(&self) -> &'static [&'static str] { ecmascript::PUBLIC_SYMBOL_KINDS }
    fn visibility_mechanism(&self) -> VisibilityMechanism { VisibilityMechanism::ExplicitExport }
    fn scope_creating_kinds(&self) -> &'static [&'static str] { ecmascript::SCOPE_CREATING_KINDS }
    fn control_flow_kinds(&self) -> &'static [&'static str] { ecmascript::CONTROL_FLOW_KINDS }
    fn complexity_nodes(&self) -> &'static [&'static str] { ecmascript::COMPLEXITY_NODES }
    fn nesting_nodes(&self) -> &'static [&'static str] { ecmascript::NESTING_NODES }

    fn extract_function(&self, node: &Node, content: &str, in_container: bool) -> Option<Symbol> {
        let name = self.node_name(node, content)?;
        Some(ecmascript::extract_function(node, content, in_container, name))
    }

    fn extract_container(&self, node: &Node, content: &str) -> Option<Symbol> {
        let name = self.node_name(node, content)?;
        Some(ecmascript::extract_container(node, name))
    }

    fn extract_imports(&self, node: &Node, content: &str) -> Vec<Import> {
        ecmascript::extract_imports(node, content)
    }

    fn extract_public_symbols(&self, node: &Node, content: &str) -> Vec<Export> {
        ecmascript::extract_public_symbols(node, content)
    }

    // === Import Resolution ===

    fn lang_key(&self) -> &'static str { "js" }

    fn resolve_local_import(
        &self,
        module: &str,
        current_file: &Path,
        _project_root: &Path,
    ) -> Option<PathBuf> {
        ecmascript::resolve_local_import(module, current_file, ecmascript::JS_EXTENSIONS)
    }

    fn resolve_external_import(&self, import_name: &str, project_root: &Path) -> Option<ResolvedPackage> {
        ecmascript::resolve_external_import(import_name, project_root)
    }

    fn get_version(&self, _project_root: &Path) -> Option<String> {
        ecmascript::get_version()
    }

    fn find_package_cache(&self, project_root: &Path) -> Option<PathBuf> {
        ecmascript::find_package_cache(project_root)
    }

    fn indexable_extensions(&self) -> &'static [&'static str] {
        &["js", "mjs", "cjs"]
    }

    fn package_sources(&self, project_root: &Path) -> Vec<crate::PackageSource> {
        use crate::{PackageSource, PackageSourceKind};
        let mut sources = Vec::new();
        if let Some(cache) = self.find_package_cache(project_root) {
            sources.push(PackageSource {
                name: "node_modules",
                path: cache,
                kind: PackageSourceKind::NpmScoped,
                version_specific: false,
            });
        }
        // Also check for Deno cache
        if let Some(deno_cache) = ecmascript::find_deno_cache() {
            let npm_cache = deno_cache.join("npm").join("registry.npmjs.org");
            if npm_cache.is_dir() {
                sources.push(PackageSource {
                    name: "deno-npm",
                    path: npm_cache,
                    kind: PackageSourceKind::Deno,
                    version_specific: false,
                });
            }
        }
        sources
    }

    fn should_skip_package_entry(&self, name: &str, is_dir: bool) -> bool {
        use crate::traits::{skip_dotfiles, has_extension};
        if skip_dotfiles(name) { return true; }
        // Skip common non-source dirs
        if is_dir && (name == "node_modules" || name == ".bin" || name == "test" || name == "tests") {
            return true;
        }
        !is_dir && !has_extension(name, &["js", "mjs", "cjs"])
    }

    fn discover_packages(&self, source: &crate::PackageSource) -> Vec<(String, PathBuf)> {
        match source.kind {
            crate::PackageSourceKind::NpmScoped => self.discover_npm_scoped_packages(&source.path),
            crate::PackageSourceKind::Deno => discover_deno_packages(&source.path),
            _ => Vec::new(),
        }
    }
}

/// Discover packages in Deno npm cache (package/version/ structure with scoped packages).
fn discover_deno_packages(source_path: &Path) -> Vec<(String, PathBuf)> {
    let entries = match std::fs::read_dir(source_path) {
        Ok(e) => e,
        Err(_) => return Vec::new(),
    };

    let mut packages = Vec::new();

    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();

        if !path.is_dir() {
            continue;
        }

        // Handle scoped packages (@scope/name)
        if name.starts_with('@') {
            if let Ok(scoped) = std::fs::read_dir(&path) {
                for scoped_entry in scoped.flatten() {
                    let scoped_path = scoped_entry.path();
                    let scoped_name = format!("{}/{}", name, scoped_entry.file_name().to_string_lossy());
                    if let Some((pkg_name, pkg_path)) = find_deno_version_dir(&scoped_path, &scoped_name) {
                        packages.push((pkg_name, pkg_path));
                    }
                }
            }
        } else if let Some((pkg_name, pkg_path)) = find_deno_version_dir(&path, &name) {
            packages.push((pkg_name, pkg_path));
        }
    }

    packages
}

/// Find the latest version directory in a Deno package directory.
fn find_deno_version_dir(pkg_path: &Path, pkg_name: &str) -> Option<(String, PathBuf)> {
    let versions: Vec<_> = std::fs::read_dir(pkg_path).ok()?
        .flatten()
        .filter(|e| e.path().is_dir())
        .collect();

    if versions.is_empty() {
        return None;
    }

    // Use the last version (sorted lexically, usually latest)
    let version_dir = versions.last()?.path();
    Some((pkg_name.to_string(), version_dir))
}
