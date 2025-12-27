//! yarn.lock parser (Yarn v1 classic format)

use crate::{DependencyTree, PackageError, TreeNode};
use std::path::Path;

/// Get installed version from yarn.lock
pub fn installed_version(package: &str, project_root: &Path) -> Option<String> {
    let lockfile = find_lockfile(project_root)?;
    let content = std::fs::read_to_string(&lockfile).ok()?;

    // yarn.lock format:
    // "package@^1.0.0":
    //   version "1.2.3"
    //   resolved "..."
    //   ...
    let mut in_package = false;
    for line in content.lines() {
        // Check if this line starts a package entry
        if line.starts_with(&format!("\"{}@", package))
            || line.starts_with(&format!("{}@", package))
        {
            in_package = true;
        } else if in_package && line.trim().starts_with("version ") {
            // Extract version from: version "1.2.3"
            let version = line.trim().strip_prefix("version ")?;
            return Some(version.trim_matches('"').to_string());
        } else if !line.starts_with(' ') && !line.is_empty() {
            // New package entry started
            in_package = false;
        }
    }

    None
}

/// Build dependency tree from yarn.lock
pub fn dependency_tree(project_root: &Path) -> Option<Result<DependencyTree, PackageError>> {
    let lockfile = find_lockfile(project_root)?;
    let _content = std::fs::read_to_string(&lockfile).ok()?;
    Some(build_tree(project_root))
}

fn find_lockfile(project_root: &Path) -> Option<std::path::PathBuf> {
    let mut current = project_root.to_path_buf();
    loop {
        let lockfile = current.join("yarn.lock");
        if lockfile.exists() {
            return Some(lockfile);
        }
        if !current.pop() {
            break;
        }
    }
    None
}

fn build_tree(project_root: &Path) -> Result<DependencyTree, PackageError> {
    // Get project info and direct dependencies from package.json
    let pkg_json = project_root.join("package.json");
    let content = std::fs::read_to_string(&pkg_json)
        .map_err(|e| PackageError::ParseError(format!("failed to read package.json: {}", e)))?;
    let pkg: serde_json::Value = serde_json::from_str(&content)
        .map_err(|e| PackageError::ParseError(format!("invalid JSON: {}", e)))?;

    let name = pkg.get("name").and_then(|n| n.as_str()).unwrap_or("root");
    let version = pkg
        .get("version")
        .and_then(|v| v.as_str())
        .unwrap_or("0.0.0");

    let mut root_deps = Vec::new();

    // Read direct dependencies from package.json
    for dep_type in ["dependencies", "devDependencies"] {
        if let Some(deps) = pkg.get(dep_type).and_then(|d| d.as_object()) {
            for (dep_name, _version_req) in deps {
                // Look up actual installed version
                let installed = installed_version(dep_name, project_root);
                root_deps.push(TreeNode {
                    name: dep_name.clone(),
                    version: installed.unwrap_or_else(|| "?".to_string()),
                    dependencies: Vec::new(), // Skip nested deps for now
                });
            }
        }
    }

    let root = TreeNode {
        name: name.to_string(),
        version: version.to_string(),
        dependencies: root_deps,
    };

    Ok(DependencyTree { roots: vec![root] })
}
