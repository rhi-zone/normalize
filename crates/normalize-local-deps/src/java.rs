//! Java local dependency discovery.

use crate::ResolvedPackage;
use crate::{LocalDepSource, LocalDepSourceKind, LocalDeps};
use crate::{has_extension, skip_dotfiles};
use std::path::{Path, PathBuf};
use std::process::Command;

// ============================================================================
// Java external package resolution
// ============================================================================

/// Get Java version.
pub fn get_java_version() -> Option<String> {
    let output = Command::new("java").args(["--version"]).output().ok()?;

    if output.status.success() {
        let version_str = String::from_utf8_lossy(&output.stdout);
        // "openjdk 17.0.1 2021-10-19" or "java 21.0.1 2023-10-17 LTS"
        for line in version_str.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                let version = parts[1];
                let ver_parts: Vec<&str> = version.split('.').collect();
                if ver_parts.len() >= 2 {
                    return Some(format!("{}.{}", ver_parts[0], ver_parts[1]));
                } else if ver_parts.len() == 1 {
                    return Some(format!("{}.0", ver_parts[0]));
                }
            }
        }
    }

    None
}

/// Find Maven local repository.
pub fn find_maven_repository() -> Option<PathBuf> {
    // Check M2_HOME or MAVEN_HOME env var
    if let Ok(m2_home) = std::env::var("M2_HOME").or_else(|_| std::env::var("MAVEN_HOME")) {
        let repo = PathBuf::from(m2_home).join("repository");
        if repo.is_dir() {
            return Some(repo);
        }
    }

    // Default ~/.m2/repository
    if let Ok(home) = std::env::var("HOME") {
        let repo = PathBuf::from(home).join(".m2").join("repository");
        if repo.is_dir() {
            return Some(repo);
        }
    }

    // Windows fallback
    if let Ok(home) = std::env::var("USERPROFILE") {
        let repo = PathBuf::from(home).join(".m2").join("repository");
        if repo.is_dir() {
            return Some(repo);
        }
    }

    None
}

/// Find Gradle cache directory.
pub fn find_gradle_cache() -> Option<PathBuf> {
    // Check GRADLE_USER_HOME env var
    if let Ok(gradle_home) = std::env::var("GRADLE_USER_HOME") {
        let cache = PathBuf::from(gradle_home)
            .join("caches")
            .join("modules-2")
            .join("files-2.1");
        if cache.is_dir() {
            return Some(cache);
        }
    }

    // Default ~/.gradle/caches/modules-2/files-2.1
    if let Ok(home) = std::env::var("HOME") {
        let cache = PathBuf::from(home)
            .join(".gradle")
            .join("caches")
            .join("modules-2")
            .join("files-2.1");
        if cache.is_dir() {
            return Some(cache);
        }
    }

    // Windows fallback
    if let Ok(home) = std::env::var("USERPROFILE") {
        let cache = PathBuf::from(home)
            .join(".gradle")
            .join("caches")
            .join("modules-2")
            .join("files-2.1");
        if cache.is_dir() {
            return Some(cache);
        }
    }

    None
}

/// Resolve a Java import to a source file in Maven/Gradle cache.
pub fn resolve_java_import(
    import: &str,
    maven_repo: Option<&Path>,
    gradle_cache: Option<&Path>,
) -> Option<ResolvedPackage> {
    let package_path = import.replace('.', "/");

    // Try Maven first
    if let Some(maven) = maven_repo
        && let Some(result) = find_java_package_in_maven(maven, &package_path, import)
    {
        return Some(result);
    }

    // Try Gradle
    if let Some(gradle) = gradle_cache
        && let Some(result) = find_java_package_in_gradle(gradle, &package_path, import)
    {
        return Some(result);
    }

    None
}

fn find_java_package_in_maven(
    maven_repo: &Path,
    package_path: &str,
    import: &str,
) -> Option<ResolvedPackage> {
    let target_dir = maven_repo.join(package_path);
    if target_dir.is_dir() {
        return find_maven_artifact(&target_dir, import);
    }

    // Try parent paths
    let parts: Vec<&str> = package_path.split('/').collect();
    for i in (2..parts.len()).rev() {
        let dir_path = parts[..i].join("/");
        let artifact = parts[i - 1];
        let search_dir = maven_repo.join(&dir_path);

        if search_dir.is_dir() {
            if let Some(result) = find_maven_artifact(&search_dir, import) {
                return Some(result);
            }
            let artifact_dir = search_dir.join(artifact);
            if artifact_dir.is_dir()
                && let Some(result) = find_maven_artifact(&artifact_dir, import)
            {
                return Some(result);
            }
        }
    }

    None
}

fn find_maven_artifact(artifact_dir: &Path, import: &str) -> Option<ResolvedPackage> {
    let versions: Vec<_> = std::fs::read_dir(artifact_dir)
        .ok()?
        .flatten()
        .filter(|e| e.path().is_dir())
        .collect();

    if versions.is_empty() {
        return None;
    }

    let mut versions: Vec<_> = versions.into_iter().collect();
    versions.sort_by(|a, b| {
        let a_name = a.file_name().to_string_lossy().to_string();
        let b_name = b.file_name().to_string_lossy().to_string();
        version_cmp(&a_name, &b_name)
    });

    let version_dir = versions.last()?.path();
    let artifact_name = artifact_dir.file_name()?.to_string_lossy().to_string();
    let version = version_dir.file_name()?.to_string_lossy().to_string();

    // Prefer sources JAR
    let sources_jar = version_dir.join(format!("{}-{}-sources.jar", artifact_name, version));
    if sources_jar.is_file() {
        return Some(ResolvedPackage {
            path: sources_jar,
            name: import.to_string(),
            is_namespace: false,
        });
    }

    // Fall back to regular JAR
    let jar = version_dir.join(format!("{}-{}.jar", artifact_name, version));
    if jar.is_file() {
        return Some(ResolvedPackage {
            path: jar,
            name: import.to_string(),
            is_namespace: false,
        });
    }

    None
}

fn find_java_package_in_gradle(
    gradle_cache: &Path,
    package_path: &str,
    import: &str,
) -> Option<ResolvedPackage> {
    let parts: Vec<&str> = package_path.split('/').collect();

    for i in (2..parts.len()).rev() {
        let group = parts[..i - 1].join(".");
        let artifact = parts[i - 1];
        let group_dir = gradle_cache.join(&group);

        if group_dir.is_dir() {
            let artifact_dir = group_dir.join(artifact);
            if artifact_dir.is_dir()
                && let Some(result) = find_gradle_artifact(&artifact_dir, import)
            {
                return Some(result);
            }
        }
    }

    None
}

fn find_gradle_artifact(artifact_dir: &Path, import: &str) -> Option<ResolvedPackage> {
    let versions: Vec<_> = std::fs::read_dir(artifact_dir)
        .ok()?
        .flatten()
        .filter(|e| e.path().is_dir())
        .collect();

    if versions.is_empty() {
        return None;
    }

    let mut versions: Vec<_> = versions.into_iter().collect();
    versions.sort_by(|a, b| {
        let a_name = a.file_name().to_string_lossy().to_string();
        let b_name = b.file_name().to_string_lossy().to_string();
        version_cmp(&a_name, &b_name)
    });

    let version_dir = versions.last()?.path();

    let hash_dirs: Vec<_> = std::fs::read_dir(&version_dir)
        .ok()?
        .flatten()
        .filter(|e| e.path().is_dir())
        .collect();

    for hash_dir in hash_dirs {
        let hash_path = hash_dir.path();

        // Look for sources JAR first
        if let Ok(entries) = std::fs::read_dir(&hash_path) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.ends_with("-sources.jar") {
                    return Some(ResolvedPackage {
                        path: entry.path(),
                        name: import.to_string(),
                        is_namespace: false,
                    });
                }
            }
        }

        // Fall back to regular JAR
        if let Ok(entries) = std::fs::read_dir(&hash_path) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.ends_with(".jar")
                    && !name.ends_with("-sources.jar")
                    && !name.ends_with("-javadoc.jar")
                {
                    return Some(ResolvedPackage {
                        path: entry.path(),
                        name: import.to_string(),
                        is_namespace: false,
                    });
                }
            }
        }
    }

    None
}

fn version_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    let a_parts: Vec<u32> = a.split('.').filter_map(|p| p.parse().ok()).collect();
    let b_parts: Vec<u32> = b.split('.').filter_map(|p| p.parse().ok()).collect();

    for (ap, bp) in a_parts.iter().zip(b_parts.iter()) {
        match ap.cmp(bp) {
            std::cmp::Ordering::Equal => continue,
            other => return other,
        }
    }
    a_parts.len().cmp(&b_parts.len())
}

/// Check if a directory contains JAR files (indicates a version directory).
fn has_jar_files(path: &Path) -> bool {
    std::fs::read_dir(path)
        .into_iter()
        .flatten()
        .flatten()
        .any(|e| e.file_name().to_string_lossy().ends_with(".jar"))
}

/// Find the main JAR in a Maven version directory.
fn find_maven_jar(version_dir: &Path, artifact: &str) -> Option<PathBuf> {
    // Prefer sources JAR
    let entries: Vec<_> = std::fs::read_dir(version_dir).ok()?.flatten().collect();

    // Look for artifact-version-sources.jar first
    for entry in &entries {
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with(artifact) && name.ends_with("-sources.jar") {
            return Some(entry.path());
        }
    }

    // Fall back to regular jar
    for entry in &entries {
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with(artifact)
            && name.ends_with(".jar")
            && !name.ends_with("-sources.jar")
            && !name.ends_with("-javadoc.jar")
        {
            return Some(entry.path());
        }
    }

    None
}

/// Discover packages in Maven repository structure.
pub fn discover_maven_packages(maven_repo: &Path, current: &Path) -> Vec<(String, PathBuf)> {
    let entries = match std::fs::read_dir(current) {
        Ok(e) => e,
        Err(_) => return Vec::new(),
    };

    let mut packages = Vec::new();

    for entry in entries.flatten() {
        let path = entry.path();

        if path.is_dir() {
            if has_jar_files(&path) {
                // This is a version directory - parent is artifact, grandparent path is group
                if let Some(artifact_dir) = current.parent() {
                    let artifact = current.file_name().unwrap_or_default().to_string_lossy();
                    if let Ok(group_path) = artifact_dir.strip_prefix(maven_repo) {
                        let group = group_path.to_string_lossy().replace('/', ".");
                        let pkg_name = format!("{}:{}", group, artifact);

                        if let Some(jar_path) = find_maven_jar(&path, &artifact) {
                            packages.push((pkg_name, jar_path));
                        }
                    }
                }
            } else {
                packages.extend(discover_maven_packages(maven_repo, &path));
            }
        }
    }

    packages
}

/// Discover packages in Gradle cache structure.
pub fn discover_gradle_packages(gradle_cache: &Path, current: &Path) -> Vec<(String, PathBuf)> {
    let entries = match std::fs::read_dir(current) {
        Ok(e) => e,
        Err(_) => return Vec::new(),
    };

    let mut packages = Vec::new();

    for entry in entries.flatten() {
        let path = entry.path();

        if path.is_dir() {
            let name = entry.file_name().to_string_lossy().to_string();
            // Check if this is a hash directory (40 hex chars)
            if name.len() == 40 && name.chars().all(|c| c.is_ascii_hexdigit()) {
                // This is a hash dir, find JAR
                if let Ok(files) = std::fs::read_dir(&path) {
                    for file in files.flatten() {
                        let file_name = file.file_name().to_string_lossy().to_string();
                        if file_name.ends_with(".jar")
                            && !file_name.ends_with("-sources.jar")
                            && !file_name.ends_with("-javadoc.jar")
                        {
                            // Extract package info from path
                            if let Ok(rel) = current.strip_prefix(gradle_cache) {
                                let parts: Vec<_> = rel.components().collect();
                                if parts.len() >= 2 {
                                    let group = parts[..parts.len() - 1]
                                        .iter()
                                        .map(|c| c.as_os_str().to_string_lossy())
                                        .collect::<Vec<_>>()
                                        .join(".");
                                    let artifact =
                                        parts.last().unwrap().as_os_str().to_string_lossy();
                                    let pkg_name = format!("{}:{}", group, artifact);
                                    packages.push((pkg_name, file.path()));
                                }
                            }
                        }
                    }
                }
            } else {
                packages.extend(discover_gradle_packages(gradle_cache, &path));
            }
        }
    }

    packages
}

// ============================================================================
// Java local deps implementation
// ============================================================================

/// Java local dependency discovery.
pub struct JavaDeps;

impl LocalDeps for JavaDeps {
    fn language_name(&self) -> &'static str {
        "Java"
    }

    fn indexable_extensions(&self) -> &'static [&'static str] {
        &["java"]
    }

    fn ecosystem_key(&self) -> &'static str {
        "java"
    }

    fn resolve_local_import(
        &self,
        import: &str,
        current_file: &Path,
        project_root: &Path,
    ) -> Option<PathBuf> {
        // Convert import to path: com.foo.Bar -> com/foo/Bar.java
        let path_part = import.replace('.', "/");

        // Common Java source directories
        let source_dirs = [
            "src/main/java",
            "src/java",
            "src",
            "app/src/main/java", // Android
        ];

        for src_dir in &source_dirs {
            let source_path = project_root
                .join(src_dir)
                .join(format!("{}.java", path_part));
            if source_path.is_file() {
                return Some(source_path);
            }
        }

        // Also try relative to current file's package structure
        // Find the source root by walking up from current file
        let mut current = current_file.parent()?;
        while current != project_root {
            // Check if this might be a source root
            let potential = current.join(format!("{}.java", path_part));
            if potential.is_file() {
                return Some(potential);
            }
            current = current.parent()?;
        }

        None
    }

    fn resolve_external_import(
        &self,
        import_name: &str,
        _project_root: &Path,
    ) -> Option<ResolvedPackage> {
        let maven_repo = find_maven_repository();
        let gradle_cache = find_gradle_cache();

        resolve_java_import(import_name, maven_repo.as_deref(), gradle_cache.as_deref())
    }

    fn is_stdlib_import(&self, import_name: &str, _project_root: &Path) -> bool {
        import_name.starts_with("java.") || import_name.starts_with("javax.")
    }

    fn get_version(&self, _project_root: &Path) -> Option<String> {
        get_java_version()
    }

    fn find_package_cache(&self, _project_root: &Path) -> Option<PathBuf> {
        find_maven_repository().or_else(find_gradle_cache)
    }

    fn find_stdlib(&self, _project_root: &Path) -> Option<PathBuf> {
        // Java stdlib is in rt.jar/modules, not easily indexable
        None
    }

    fn dep_sources(&self, _project_root: &Path) -> Vec<LocalDepSource> {
        let mut sources = Vec::new();
        if let Some(maven) = find_maven_repository() {
            sources.push(LocalDepSource {
                name: "maven",
                path: maven,
                kind: LocalDepSourceKind::Maven,
                version_specific: false,
            });
        }
        if let Some(gradle) = find_gradle_cache() {
            sources.push(LocalDepSource {
                name: "gradle",
                path: gradle,
                kind: LocalDepSourceKind::Gradle,
                version_specific: false,
            });
        }
        sources
    }

    fn should_skip_dep_entry(&self, name: &str, is_dir: bool) -> bool {
        if skip_dotfiles(name) {
            return true;
        }
        // Skip META-INF, test dirs
        if is_dir && (name == "META-INF" || name == "test" || name == "tests") {
            return true;
        }
        !is_dir && !has_extension(name, self.indexable_extensions())
    }

    fn discover_packages(&self, source: &LocalDepSource) -> Vec<(String, PathBuf)> {
        match source.kind {
            LocalDepSourceKind::Maven => discover_maven_packages(&source.path, &source.path),
            LocalDepSourceKind::Gradle => discover_gradle_packages(&source.path, &source.path),
            _ => Vec::new(),
        }
    }

    fn dep_module_name(&self, entry_name: &str) -> String {
        entry_name
            .strip_suffix(".java")
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
        if path.extension()?.to_str()? != "java" {
            return None;
        }
        // Java: com/foo/Bar.java -> com.foo.Bar
        let path_str = path.to_str()?;
        // Remove common source prefixes
        let rel = path_str
            .strip_prefix("src/main/java/")
            .or_else(|| path_str.strip_prefix("src/"))
            .unwrap_or(path_str);
        let without_ext = rel.strip_suffix(".java")?;
        Some(without_ext.replace('/', "."))
    }

    fn module_name_to_paths(&self, module: &str) -> Vec<String> {
        let path = module.replace('.', "/");
        vec![
            format!("src/main/java/{}.java", path),
            format!("src/{}.java", path),
        ]
    }
}
