//! Scala local dependency discovery.

use crate::{LocalDepSource, LocalDeps};
use crate::{has_extension, skip_dotfiles};
use std::path::{Path, PathBuf};

/// Scala local dependency discovery (sbt/Maven/Gradle builds).
pub struct ScalaDeps;

impl LocalDeps for ScalaDeps {
    fn language_name(&self) -> &'static str {
        "Scala"
    }

    fn ecosystem_key(&self) -> &'static str {
        "scala"
    }

    fn project_manifest_filenames(&self) -> &'static [&'static str] {
        &["build.sbt", "pom.xml", "build.gradle", "build.gradle.kts"]
    }

    fn indexable_extensions(&self) -> &'static [&'static str] {
        &["scala", "sc"]
    }

    fn discover_workspace_members(&self, root: &Path) -> Vec<PathBuf> {
        // sbt workspace members are declared in the root build.sbt as:
        //   lazy val foo = project in file("engine/foo")
        //   lazy val foo = (project in file("engine/foo"))
        // Parse all `file("path")` occurrences.
        let build_sbt = root.join("build.sbt");
        if !build_sbt.is_file() {
            return Vec::new();
        }
        let content = match std::fs::read_to_string(&build_sbt) {
            Ok(c) => c,
            Err(_) => return Vec::new(),
        };
        let mut dirs = Vec::new();
        let mut rest = content.as_str();
        while let Some(idx) = rest.find("file(\"") {
            rest = &rest[idx + 6..]; // skip `file("`
            if let Some(end) = rest.find('"') {
                let path = rest[..end].trim_end_matches('/');
                if !path.is_empty() && path != "." {
                    let candidate = root.join(path);
                    if candidate.is_dir() {
                        dirs.push(candidate);
                    }
                }
                rest = &rest[end + 1..];
            }
        }
        dirs
    }

    fn resolve_local_import(
        &self,
        import: &str,
        _current_file: &Path,
        project_root: &Path,
    ) -> Option<PathBuf> {
        // Strip any wildcard or multi-import suffixes
        let base = import.trim_end_matches("._").trim_end_matches(".*");
        let path_part = base.replace('.', "/");

        for src_dir in &[
            "src/main/scala",
            "src/main/java", // Scala can coexist with Java
            "src/scala",
            "src",
        ] {
            for ext in &["scala", "sc"] {
                let p = project_root
                    .join(src_dir)
                    .join(format!("{}.{}", path_part, ext));
                if p.is_file() {
                    return Some(p);
                }
            }
        }
        None
    }

    fn is_stdlib_import(&self, import_name: &str, _project_root: &Path) -> bool {
        import_name.starts_with("scala.")
            || import_name.starts_with("java.")
            || import_name.starts_with("javax.")
    }

    fn get_version(&self, _project_root: &Path) -> Option<String> {
        crate::java::get_java_version()
    }

    fn find_package_cache(&self, _project_root: &Path) -> Option<PathBuf> {
        // Scala/sbt uses Ivy2 cache and Coursier cache.
        let home = std::env::var("HOME").ok()?;
        let coursier = PathBuf::from(&home).join(".cache/coursier/v1/https");
        if coursier.is_dir() {
            return Some(coursier);
        }
        let ivy2 = PathBuf::from(&home).join(".ivy2/cache");
        if ivy2.is_dir() {
            return Some(ivy2);
        }
        crate::java::find_maven_repository()
    }

    fn dep_sources(&self, project_root: &Path) -> Vec<LocalDepSource> {
        let mut sources = Vec::new();
        if let Some(cache) = self.find_package_cache(project_root) {
            sources.push(LocalDepSource {
                name: "ivy2/coursier",
                path: cache,
                kind: crate::LocalDepSourceKind::Flat,
                version_specific: true,
            });
        }
        sources
    }

    fn should_skip_dep_entry(&self, name: &str, is_dir: bool) -> bool {
        if skip_dotfiles(name) {
            return true;
        }
        if is_dir && (name == "META-INF" || name == "test" || name == "tests") {
            return true;
        }
        !is_dir && !has_extension(name, self.indexable_extensions())
    }

    fn file_path_to_module_name(&self, path: &Path) -> Option<String> {
        let ext = path.extension()?.to_str()?;
        if ext != "scala" && ext != "sc" {
            return None;
        }
        let path_str = path.to_str()?;
        let rel = path_str
            .strip_prefix("src/main/scala/")
            .or_else(|| path_str.strip_prefix("src/scala/"))
            .or_else(|| path_str.strip_prefix("src/"))
            .unwrap_or(path_str);
        let without_ext = rel
            .strip_suffix(".scala")
            .or_else(|| rel.strip_suffix(".sc"))?;
        Some(without_ext.replace('/', "."))
    }

    fn module_name_to_paths(&self, module: &str) -> Vec<String> {
        let path = module.replace('.', "/");
        vec![
            format!("src/main/scala/{}.scala", path),
            format!("src/scala/{}.scala", path),
            format!("src/{}.scala", path),
        ]
    }
}
