//! Kotlin local dependency discovery.

use crate::ResolvedPackage;
use crate::java::{find_gradle_cache, find_maven_repository, get_java_version};
use crate::{LocalDepSource, LocalDeps};
use crate::{has_extension, skip_dotfiles};
use std::path::{Path, PathBuf};

/// Kotlin local dependency discovery.
pub struct KotlinDeps;

impl LocalDeps for KotlinDeps {
    fn language_name(&self) -> &'static str {
        "Kotlin"
    }

    fn indexable_extensions(&self) -> &'static [&'static str] {
        &["kt", "kts"]
    }

    fn ecosystem_key(&self) -> &'static str {
        "kotlin"
    }

    fn resolve_local_import(
        &self,
        import: &str,
        current_file: &Path,
        project_root: &Path,
    ) -> Option<PathBuf> {
        let path_part = import.replace('.', "/");

        // Common Kotlin source directories
        let source_dirs = [
            "src/main/kotlin",
            "src/main/java", // Kotlin can live alongside Java
            "src/kotlin",
            "src",
            "app/src/main/kotlin", // Android
            "app/src/main/java",
        ];

        for src_dir in &source_dirs {
            // Try .kt first, then .java (Kotlin can import Java)
            for ext in &["kt", "java"] {
                let source_path = project_root
                    .join(src_dir)
                    .join(format!("{}.{}", path_part, ext));
                if source_path.is_file() {
                    return Some(source_path);
                }
            }
        }

        // Also try relative to current file's package structure
        let mut current = current_file.parent()?;
        while current != project_root {
            for ext in &["kt", "java"] {
                let potential = current.join(format!("{}.{}", path_part, ext));
                if potential.is_file() {
                    return Some(potential);
                }
            }
            current = current.parent()?;
        }

        None
    }

    fn resolve_external_import(
        &self,
        import_name: &str,
        project_root: &Path,
    ) -> Option<ResolvedPackage> {
        // Kotlin uses Maven/Gradle like Java
        // Reuse Java's resolution (they share the same cache)
        crate::java::JavaDeps.resolve_external_import(import_name, project_root)
    }

    fn is_stdlib_import(&self, import_name: &str, _project_root: &Path) -> bool {
        import_name.starts_with("kotlin.")
            || import_name.starts_with("kotlinx.")
            || import_name.starts_with("java.")
            || import_name.starts_with("javax.")
    }

    fn get_version(&self, _project_root: &Path) -> Option<String> {
        // Use Java version as proxy (Kotlin runs on JVM)
        get_java_version()
    }

    fn find_package_cache(&self, _project_root: &Path) -> Option<PathBuf> {
        find_maven_repository().or_else(find_gradle_cache)
    }

    fn find_stdlib(&self, _project_root: &Path) -> Option<PathBuf> {
        // Kotlin stdlib is bundled with the compiler/runtime
        None
    }

    fn dep_sources(&self, _project_root: &Path) -> Vec<LocalDepSource> {
        // Reuse Java's package sources (shared Maven/Gradle cache)
        crate::java::JavaDeps.dep_sources(_project_root)
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

    fn discover_packages(&self, source: &LocalDepSource) -> Vec<(String, PathBuf)> {
        // Reuse Java's package discovery
        crate::java::JavaDeps.discover_packages(source)
    }

    fn dep_module_name(&self, entry_name: &str) -> String {
        entry_name
            .strip_suffix(".kt")
            .or_else(|| entry_name.strip_suffix(".kts"))
            .unwrap_or(entry_name)
            .to_string()
    }

    fn find_package_entry(&self, path: &Path) -> Option<PathBuf> {
        if path.is_file() {
            return Some(path.to_path_buf());
        }
        // For JAR files, return the JAR itself
        if path.extension().map(|e| e == "jar").unwrap_or(false) {
            return Some(path.to_path_buf());
        }
        None
    }

    fn file_path_to_module_name(&self, path: &Path) -> Option<String> {
        let ext = path.extension()?.to_str()?;
        if ext != "kt" && ext != "kts" {
            return None;
        }
        // Kotlin: com/foo/Bar.kt -> com.foo.Bar
        let path_str = path.to_str()?;
        // Remove common source prefixes
        let rel = path_str
            .strip_prefix("src/main/kotlin/")
            .or_else(|| path_str.strip_prefix("src/main/java/"))
            .or_else(|| path_str.strip_prefix("src/"))
            .unwrap_or(path_str);
        let without_ext = rel
            .strip_suffix(".kt")
            .or_else(|| rel.strip_suffix(".kts"))?;
        Some(without_ext.replace('/', "."))
    }

    fn module_name_to_paths(&self, module: &str) -> Vec<String> {
        let path = module.replace('.', "/");
        vec![
            format!("src/main/kotlin/{}.kt", path),
            format!("src/main/java/{}.kt", path), // Kotlin can live in java dirs
            format!("src/{}.kt", path),
        ]
    }
}
