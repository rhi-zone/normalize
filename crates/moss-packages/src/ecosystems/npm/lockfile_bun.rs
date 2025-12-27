//! bun.lock (text) and bun.lockb (binary) parser
//!
//! Binary format ported from Bun (MIT License):
//! Copyright (c) 2022 Oven-sh
//! https://github.com/oven-sh/bun/blob/main/src/install/lockfile.zig

use crate::{DependencyTree, PackageError, TreeNode};
use std::path::Path;

/// Get installed version from bun.lock or bun.lockb
pub fn installed_version(package: &str, project_root: &Path) -> Option<String> {
    // Try text format first (bun.lock)
    if let Some(v) = installed_version_text(package, project_root) {
        return Some(v);
    }
    // Fall back to binary format
    installed_version_binary(package, project_root)
}

fn installed_version_text(package: &str, project_root: &Path) -> Option<String> {
    let lockfile = find_text_lockfile(project_root)?;
    let content = std::fs::read_to_string(&lockfile).ok()?;
    let parsed: serde_json::Value = serde_json_lenient::from_str(&content).ok()?;

    // packages section: "pkg": ["pkg@version", registry, {deps}, hash]
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

    // Also check workspaces for direct deps
    if let Some(workspaces) = parsed.get("workspaces").and_then(|w| w.as_object()) {
        for (_ws_path, ws_info) in workspaces {
            for dep_type in ["dependencies", "devDependencies", "optionalDependencies"] {
                if let Some(deps) = ws_info.get(dep_type).and_then(|d| d.as_object()) {
                    if deps.contains_key(package) {
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
    let lockfile = find_binary_lockfile(project_root)?;
    let data = std::fs::read(&lockfile).ok()?;
    let parsed = BunLockb::parse(&data)?;
    parsed.find_package_version(package)
}

/// Build dependency tree from bun.lock or bun.lockb
pub fn dependency_tree(project_root: &Path) -> Option<Result<DependencyTree, PackageError>> {
    // Try text format first
    if let Some(lockfile) = find_text_lockfile(project_root) {
        let content = std::fs::read_to_string(&lockfile).ok()?;
        let parsed: serde_json::Value = serde_json_lenient::from_str(&content).ok()?;
        return Some(build_tree_text(&parsed, project_root));
    }

    // Try binary format
    if let Some(lockfile) = find_binary_lockfile(project_root) {
        if lockfile.exists() {
            return Some(build_tree_binary(project_root));
        }
    }

    None
}

// ============================================================================
// Binary format parser (bun.lockb)
// ============================================================================

/// Header magic for bun.lockb files
const HEADER_MAGIC: &[u8] = b"#!/usr/bin/env bun\nbun-lockfile-format-v0\n";

/// Parsed bun.lockb file
struct BunLockb {
    packages: Vec<BunPackage>,
}

#[derive(Debug, Clone)]
struct BunPackage {
    name: String,
    version: String,
}

impl BunLockb {
    fn parse(data: &[u8]) -> Option<Self> {
        // Validate header
        if data.len() < HEADER_MAGIC.len() + 100 {
            return None;
        }
        if !data.starts_with(HEADER_MAGIC) {
            return None;
        }

        let mut offset = HEADER_MAGIC.len(); // 42

        // Format version (u32 LE)
        let format_version = read_u32_le(data, &mut offset)?;
        if format_version > 10 {
            return None;
        }

        // Skip meta_hash (32 bytes) + total_buffer_size (8 bytes)
        offset = 0x56; // packages_count location based on format analysis

        // Read packages count
        let packages_count = u64::from_le_bytes(data[offset..offset + 8].try_into().ok()?) as usize;

        if packages_count == 0 || packages_count > 100_000 {
            return None;
        }

        // Skip to packages data start (after MultiArrayList header)
        // Header: count (u64) + alignment (u64) + begin (u64) + end (u64) = 32 bytes
        offset = 0x6e;
        let pkg_begin = u64::from_le_bytes(data[offset..offset + 8].try_into().ok()?) as usize;
        offset += 8;
        let pkg_end = u64::from_le_bytes(data[offset..offset + 8].try_into().ok()?) as usize;

        if pkg_begin >= pkg_end || pkg_end > data.len() {
            return None;
        }

        // Each package name is a String (8 bytes), stored contiguously in MultiArrayList
        // names[0..count] at pkg_begin, then name_hashes[0..count], then resolutions...
        let names_end = pkg_begin + packages_count * 8;
        if names_end > pkg_end {
            return None;
        }

        let packages = Self::extract_inline_packages(data, pkg_begin, names_end);
        Some(Self { packages })
    }

    /// Extract packages with inline names (≤7 chars).
    /// TODO: External strings require finding the string_bytes buffer location.
    fn extract_inline_packages(data: &[u8], start: usize, end: usize) -> Vec<BunPackage> {
        let mut packages = Vec::new();
        let mut offset = start;

        while offset + 8 <= end {
            let bytes: [u8; 8] = match data[offset..offset + 8].try_into() {
                Ok(b) => b,
                Err(_) => break,
            };

            // Inline string: byte[7] < 0x80
            if bytes[7] < 0x80 {
                let end_pos = bytes.iter().position(|&b| b == 0).unwrap_or(8);
                if end_pos > 0 && end_pos <= 7 {
                    if let Ok(name) = std::str::from_utf8(&bytes[..end_pos]) {
                        if Self::is_valid_package_name(name) {
                            packages.push(BunPackage {
                                name: name.to_string(),
                                version: "0.0.0".to_string(),
                            });
                        }
                    }
                }
            }
            // Skip external strings for now - they require string_bytes buffer

            offset += 8;
        }

        packages
    }

    fn is_valid_package_name(name: &str) -> bool {
        !name.is_empty()
            && name.len() <= 214
            && name
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '@' || c == '/')
    }

    fn find_package_version(&self, package: &str) -> Option<String> {
        self.packages
            .iter()
            .find(|p| p.name == package)
            .map(|p| p.version.clone())
    }

    fn to_tree(&self, project_root: &Path) -> DependencyTree {
        let (name, version) = get_project_info_from_package_json(project_root);
        let root_deps: Vec<TreeNode> = self
            .packages
            .iter()
            .map(|p| TreeNode {
                name: p.name.clone(),
                version: p.version.clone(),
                dependencies: Vec::new(),
            })
            .collect();

        DependencyTree {
            roots: vec![TreeNode {
                name,
                version,
                dependencies: root_deps,
            }],
        }
    }
}

fn read_u32_le(data: &[u8], offset: &mut usize) -> Option<u32> {
    if *offset + 4 > data.len() {
        return None;
    }
    let bytes: [u8; 4] = data[*offset..*offset + 4].try_into().ok()?;
    *offset += 4;
    Some(u32::from_le_bytes(bytes))
}

// ============================================================================
// Text format parser (bun.lock)
// ============================================================================

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
    let (name, version) = get_project_info(parsed, project_root);

    let mut root_deps = Vec::new();

    // Get direct dependencies from root workspace
    if let Some(workspaces) = parsed.get("workspaces").and_then(|w| w.as_object()) {
        if let Some(root_ws) = workspaces.get("") {
            for dep_type in ["dependencies", "devDependencies"] {
                if let Some(deps) = root_ws.get(dep_type).and_then(|d| d.as_object()) {
                    for (dep_name, _version_req) in deps {
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
    let lockfile = find_binary_lockfile(project_root)
        .ok_or_else(|| PackageError::ParseError("bun.lockb not found".to_string()))?;

    let data = std::fs::read(&lockfile)
        .map_err(|e| PackageError::ParseError(format!("failed to read bun.lockb: {}", e)))?;

    let parsed = BunLockb::parse(&data)
        .ok_or_else(|| PackageError::ParseError("invalid bun.lockb format".to_string()))?;

    Ok(parsed.to_tree(project_root))
}

fn get_project_info(parsed: &serde_json::Value, project_root: &Path) -> (String, String) {
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

    #[test]
    fn test_is_valid_package_name() {
        assert!(BunLockb::is_valid_package_name("elysia"));
        assert!(BunLockb::is_valid_package_name("vue"));
        assert!(BunLockb::is_valid_package_name("@types/node"));
        assert!(!BunLockb::is_valid_package_name(""));
        assert!(!BunLockb::is_valid_package_name("has space"));
    }

    #[test]
    fn test_parse_real_lockb() {
        let path = std::path::Path::new("/home/me/git/tinkerbox/bun.lockb");
        if !path.exists() {
            eprintln!("Skipping test: bun.lockb not found at {:?}", path);
            return;
        }

        let data = std::fs::read(path).expect("failed to read bun.lockb");
        eprintln!("Read {} bytes from bun.lockb", data.len());

        let parsed = BunLockb::parse(&data);
        if let Some(lockb) = parsed {
            eprintln!("Parsed {} packages", lockb.packages.len());
            for pkg in lockb.packages.iter().take(20) {
                eprintln!("  {} @ {}", pkg.name, pkg.version);
            }
            assert!(
                !lockb.packages.is_empty(),
                "should have found some packages"
            );
        } else {
            panic!("Failed to parse bun.lockb");
        }
    }
}
