//! bun.lock (text) and bun.lockb (binary) parser
//!
//! Binary format reference from Bun (MIT License):
//! Copyright (c) 2022 Oven-sh
//! https://github.com/oven-sh/bun/blob/main/src/install/lockfile.zig

use crate::{DependencyTree, PackageError, TreeNode};
use json_strip_comments::strip;
use std::path::Path;
use std::process::Command;

/// Get installed version from bun.lock or bun.lockb
pub fn installed_version(package: &str, project_root: &Path) -> Option<String> {
    // Try text format first (bun.lock)
    if let Some(v) = installed_version_text(package, project_root) {
        return Some(v);
    }
    // Fall back to binary format via bun CLI
    installed_version_binary(package, project_root)
}

fn installed_version_text(package: &str, project_root: &Path) -> Option<String> {
    let lockfile = find_text_lockfile(project_root)?;
    let mut content = std::fs::read_to_string(&lockfile).ok()?;
    strip(&mut content).ok()?;
    let parsed: serde_json::Value = serde_json::from_str(&content).ok()?;

    // packages section: "pkg": ["pkg@version", registry, {deps}, hash]
    if let Some(packages) = parsed.get("packages").and_then(|p| p.as_object()) {
        if let Some(pkg_info) = packages.get(package) {
            if let Some(arr) = pkg_info.as_array() {
                if let Some(first) = arr.first().and_then(|v| v.as_str()) {
                    // Parse "pkg@version" or "@scope/pkg@version"
                    if let Some(version) = extract_version_from_spec(first) {
                        return Some(version);
                    }
                }
            }
        }
    }

    // Also check workspaces for direct deps
    if let Some(workspaces) = parsed.get("workspaces").and_then(|w| w.as_object()) {
        for (_ws_path, ws_info) in workspaces {
            for dep_type in ["dependencies", "devDependencies", "optionalDependencies"] {
                if let Some(deps) = ws_info.get(dep_type).and_then(|d| d.as_object()) {
                    if deps.contains_key(package) {
                        // Found in manifest, look up in packages
                        if let Some(packages) = parsed.get("packages").and_then(|p| p.as_object()) {
                            if let Some(pkg_info) = packages.get(package) {
                                if let Some(arr) = pkg_info.as_array() {
                                    if let Some(first) = arr.first().and_then(|v| v.as_str()) {
                                        if let Some(version) = extract_version_from_spec(first) {
                                            return Some(version);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    None
}

fn installed_version_binary(package: &str, project_root: &Path) -> Option<String> {
    // Check if bun.lockb exists
    let lockfile = project_root.join("bun.lockb");
    if !lockfile.exists() {
        return None;
    }

    // Use bun pm ls to get package info
    let output = Command::new("bun")
        .args(["pm", "ls"])
        .current_dir(project_root)
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    // Parse output like:
    // /path node_modules (N)
    // ├── pkg@version
    // └── pkg2@version
    for line in stdout.lines() {
        let line = line.trim_start_matches(['├', '─', '└', '│', ' ']);
        if line.starts_with(package) {
            if let Some(at_pos) = line.rfind('@') {
                if &line[..at_pos] == package {
                    return Some(line[at_pos + 1..].to_string());
                }
            }
        }
    }

    None
}

/// Build dependency tree from bun.lock or bun.lockb
pub fn dependency_tree(project_root: &Path) -> Option<Result<DependencyTree, PackageError>> {
    // Try text format first
    if let Some(lockfile) = find_text_lockfile(project_root) {
        let mut content = std::fs::read_to_string(&lockfile).ok()?;
        strip(&mut content).ok()?;
        let parsed: serde_json::Value = serde_json::from_str(&content).ok()?;
        return Some(build_tree_text(&parsed, project_root));
    }

    // Try binary format via CLI
    let lockfile = find_binary_lockfile(project_root)?;
    if lockfile.exists() {
        return Some(build_tree_binary(project_root));
    }

    None
}

fn find_text_lockfile(project_root: &Path) -> Option<std::path::PathBuf> {
    let mut current = project_root.to_path_buf();
    loop {
        let lockfile = current.join("bun.lock");
        if lockfile.exists() {
            return Some(lockfile);
        }
        if !current.pop() {
            break;
        }
    }
    None
}

fn find_binary_lockfile(project_root: &Path) -> Option<std::path::PathBuf> {
    let mut current = project_root.to_path_buf();
    loop {
        let lockfile = current.join("bun.lockb");
        if lockfile.exists() {
            return Some(lockfile);
        }
        if !current.pop() {
            break;
        }
    }
    None
}

fn extract_version_from_spec(spec: &str) -> Option<String> {
    // Handle "@scope/pkg@version" or "pkg@version"
    if spec.starts_with('@') {
        // Scoped package: find second @
        let first_slash = spec.find('/')?;
        let version_at = spec[first_slash..].find('@').map(|i| i + first_slash)?;
        Some(spec[version_at + 1..].to_string())
    } else {
        let at_pos = spec.find('@')?;
        Some(spec[at_pos + 1..].to_string())
    }
}

fn build_tree_text(
    parsed: &serde_json::Value,
    project_root: &Path,
) -> Result<DependencyTree, PackageError> {
    // Get project info from package.json or root workspace
    let (name, version) = get_project_info(parsed, project_root);

    let mut root_deps = Vec::new();

    // Get direct dependencies from root workspace
    if let Some(workspaces) = parsed.get("workspaces").and_then(|w| w.as_object()) {
        if let Some(root_ws) = workspaces.get("") {
            for dep_type in ["dependencies", "devDependencies"] {
                if let Some(deps) = root_ws.get(dep_type).and_then(|d| d.as_object()) {
                    for (dep_name, _version_req) in deps {
                        // Look up resolved version in packages
                        let version = if let Some(packages) =
                            parsed.get("packages").and_then(|p| p.as_object())
                        {
                            packages
                                .get(dep_name)
                                .and_then(|p| p.as_array())
                                .and_then(|arr| arr.first())
                                .and_then(|v| v.as_str())
                                .and_then(extract_version_from_spec)
                                .unwrap_or_else(|| "?".to_string())
                        } else {
                            "?".to_string()
                        };

                        root_deps.push(TreeNode {
                            name: dep_name.clone(),
                            version,
                            dependencies: Vec::new(),
                        });
                    }
                }
            }
        }

        // Also add workspace packages
        for (ws_path, ws_info) in workspaces {
            if ws_path.is_empty() {
                continue;
            }
            if let Some(ws_name) = ws_info.get("name").and_then(|n| n.as_str()) {
                let ws_version = ws_info
                    .get("version")
                    .and_then(|v| v.as_str())
                    .unwrap_or("0.0.0");
                root_deps.push(TreeNode {
                    name: ws_name.to_string(),
                    version: ws_version.to_string(),
                    dependencies: Vec::new(),
                });
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

fn build_tree_binary(project_root: &Path) -> Result<DependencyTree, PackageError> {
    // Use bun pm ls to get dependency tree
    let output = Command::new("bun")
        .args(["pm", "ls"])
        .current_dir(project_root)
        .output()
        .map_err(|e| PackageError::ToolFailed(format!("bun pm ls failed: {}", e)))?;

    if !output.status.success() {
        return Err(PackageError::ToolFailed(
            "bun pm ls returned non-zero".to_string(),
        ));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let (name, version) = get_project_info_from_package_json(project_root);

    let mut root_deps = Vec::new();

    // Parse output:
    // /path node_modules (N)
    // ├── pkg@version
    // └── pkg2@version
    for line in stdout.lines().skip(1) {
        // Skip first line (path)
        let line = line.trim_start_matches(['├', '─', '└', '│', ' ']);
        if line.is_empty() {
            continue;
        }

        if let Some(at_pos) = line.rfind('@') {
            let pkg_name = &line[..at_pos];
            let pkg_version = &line[at_pos + 1..];
            root_deps.push(TreeNode {
                name: pkg_name.to_string(),
                version: pkg_version.to_string(),
                dependencies: Vec::new(),
            });
        }
    }

    let root = TreeNode {
        name,
        version,
        dependencies: root_deps,
    };

    Ok(DependencyTree { roots: vec![root] })
}

fn get_project_info(parsed: &serde_json::Value, project_root: &Path) -> (String, String) {
    // Try root workspace first
    if let Some(workspaces) = parsed.get("workspaces").and_then(|w| w.as_object()) {
        if let Some(root_ws) = workspaces.get("") {
            let name = root_ws
                .get("name")
                .and_then(|n| n.as_str())
                .unwrap_or("root");
            let version = root_ws
                .get("version")
                .and_then(|v| v.as_str())
                .unwrap_or("0.0.0");
            return (name.to_string(), version.to_string());
        }
    }

    // Fall back to package.json
    get_project_info_from_package_json(project_root)
}

fn get_project_info_from_package_json(project_root: &Path) -> (String, String) {
    let pkg_json = project_root.join("package.json");
    if let Ok(content) = std::fs::read_to_string(&pkg_json) {
        if let Ok(pkg) = serde_json::from_str::<serde_json::Value>(&content) {
            let name = pkg
                .get("name")
                .and_then(|n| n.as_str())
                .unwrap_or("root")
                .to_string();
            let version = pkg
                .get("version")
                .and_then(|v| v.as_str())
                .unwrap_or("0.0.0")
                .to_string();
            return (name, version);
        }
    }
    ("root".to_string(), "0.0.0".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_version_simple() {
        assert_eq!(
            extract_version_from_spec("react@18.2.0"),
            Some("18.2.0".to_string())
        );
    }

    #[test]
    fn test_extract_version_scoped() {
        assert_eq!(
            extract_version_from_spec("@types/node@20.0.0"),
            Some("20.0.0".to_string())
        );
    }
}
