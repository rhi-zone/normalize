//! pnpm-lock.yaml parser

use crate::{DependencyTree, PackageError, TreeNode};
use std::path::Path;

/// Get installed version from pnpm-lock.yaml
pub fn installed_version(package: &str, project_root: &Path) -> Option<String> {
    let lockfile = find_lockfile(project_root)?;
    let content = std::fs::read_to_string(&lockfile).ok()?;
    let parsed: serde_yaml::Value = serde_yaml::from_str(&content).ok()?;

    // Check packages section for the package
    // Format: packages["package@version"] or packages["/package@version"]
    if let Some(packages) = parsed.get("packages").and_then(|p| p.as_mapping()) {
        for (key, _value) in packages {
            if let Some(key_str) = key.as_str() {
                // Keys are like "@scope/pkg@1.0.0" or "pkg@1.0.0"
                let key_trimmed = key_str.trim_start_matches('/');
                if let Some((name, version)) = parse_package_key(key_trimmed) {
                    if name == package {
                        return Some(version);
                    }
                }
            }
        }
    }

    // Also check importers for direct dependencies
    if let Some(importers) = parsed.get("importers").and_then(|i| i.as_mapping()) {
        for (_importer_path, importer) in importers {
            for dep_type in ["dependencies", "devDependencies", "optionalDependencies"] {
                if let Some(deps) = importer.get(dep_type).and_then(|d| d.as_mapping()) {
                    if let Some(dep) = deps.get(package) {
                        if let Some(version_info) = dep.get("version").and_then(|v| v.as_str()) {
                            // Version might have peer dep suffix like "1.0.0(peer@2.0.0)"
                            let version = version_info.split('(').next().unwrap_or(version_info);
                            return Some(version.to_string());
                        }
                    }
                }
            }
        }
    }

    None
}

/// Build dependency tree from pnpm-lock.yaml
pub fn dependency_tree(project_root: &Path) -> Option<Result<DependencyTree, PackageError>> {
    let lockfile = find_lockfile(project_root)?;
    let content = std::fs::read_to_string(&lockfile).ok()?;
    let parsed: serde_yaml::Value = serde_yaml::from_str(&content).ok()?;
    Some(build_tree(&parsed, project_root))
}

fn find_lockfile(project_root: &Path) -> Option<std::path::PathBuf> {
    let mut current = project_root.to_path_buf();
    loop {
        let lockfile = current.join("pnpm-lock.yaml");
        if lockfile.exists() {
            return Some(lockfile);
        }
        if !current.pop() {
            break;
        }
    }
    None
}

/// Parse package key like "@scope/pkg@1.0.0" into (name, version)
fn parse_package_key(key: &str) -> Option<(String, String)> {
    // Handle scoped packages: @scope/pkg@version
    if key.starts_with('@') {
        // Find the second @ which separates name from version
        let first_slash = key.find('/')?;
        let version_at = key[first_slash..].find('@').map(|i| i + first_slash)?;
        let name = &key[..version_at];
        let version = &key[version_at + 1..];
        Some((name.to_string(), version.to_string()))
    } else {
        // Non-scoped: pkg@version
        let at_pos = key.find('@')?;
        let name = &key[..at_pos];
        let version = &key[at_pos + 1..];
        Some((name.to_string(), version.to_string()))
    }
}

fn build_tree(
    parsed: &serde_yaml::Value,
    project_root: &Path,
) -> Result<DependencyTree, PackageError> {
    // Get project name from package.json
    let pkg_json = project_root.join("package.json");
    let (name, version) = if let Ok(content) = std::fs::read_to_string(&pkg_json) {
        if let Ok(pkg) = serde_json::from_str::<serde_json::Value>(&content) {
            (
                pkg.get("name")
                    .and_then(|n| n.as_str())
                    .unwrap_or("root")
                    .to_string(),
                pkg.get("version")
                    .and_then(|v| v.as_str())
                    .unwrap_or("0.0.0")
                    .to_string(),
            )
        } else {
            ("root".to_string(), "0.0.0".to_string())
        }
    } else {
        ("root".to_string(), "0.0.0".to_string())
    };

    let mut root_deps = Vec::new();

    // Get direct dependencies from importers section
    if let Some(importers) = parsed.get("importers").and_then(|i| i.as_mapping()) {
        // Root importer is "."
        if let Some(root_importer) = importers
            .get(".")
            .or_else(|| importers.get(&serde_yaml::Value::String(".".to_string())))
        {
            for dep_type in ["dependencies", "devDependencies"] {
                if let Some(deps) = root_importer.get(dep_type).and_then(|d| d.as_mapping()) {
                    for (dep_name, dep_info) in deps {
                        if let (Some(name), Some(version_info)) = (
                            dep_name.as_str(),
                            dep_info.get("version").and_then(|v| v.as_str()),
                        ) {
                            // Version might have peer dep suffix
                            let version = version_info.split('(').next().unwrap_or(version_info);
                            root_deps.push(TreeNode {
                                name: name.to_string(),
                                version: version.to_string(),
                                dependencies: Vec::new(), // pnpm flattens deps, skip nested for now
                            });
                        }
                    }
                }
            }
        }
    }

    let root = TreeNode {
        name,
        version,
        dependencies: root_deps,
    };

    Ok(DependencyTree { roots: vec![root] })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_package_key_simple() {
        let (name, version) = parse_package_key("react@18.2.0").unwrap();
        assert_eq!(name, "react");
        assert_eq!(version, "18.2.0");
    }

    #[test]
    fn test_parse_package_key_scoped() {
        let (name, version) = parse_package_key("@types/node@20.0.0").unwrap();
        assert_eq!(name, "@types/node");
        assert_eq!(version, "20.0.0");
    }
}
