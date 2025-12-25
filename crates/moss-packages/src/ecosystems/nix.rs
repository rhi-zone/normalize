//! Nix ecosystem.

use crate::{PackageQuery, Ecosystem, LockfileManager, PackageError, PackageInfo};
use std::path::Path;
use std::process::Command;

pub struct Nix;

impl Ecosystem for Nix {
    fn name(&self) -> &'static str {
        "nix"
    }

    fn manifest_files(&self) -> &'static [&'static str] {
        &["flake.nix", "default.nix", "shell.nix"]
    }

    fn lockfiles(&self) -> &'static [LockfileManager] {
        &[LockfileManager {
            filename: "flake.lock",
            manager: "nix",
        }]
    }

    fn tools(&self) -> &'static [&'static str] {
        &["nix"]
    }

    fn fetch_info(&self, query: &PackageQuery, _tool: &str) -> Result<PackageInfo, PackageError> {
        fetch_nix_info(&query.name)
    }

    fn installed_version(&self, _package: &str, _project_root: &Path) -> Option<String> {
        // flake.lock contains input revisions, not package versions
        // Nix packages are pinned by nixpkgs revision, not individual versions
        None
    }
}

fn fetch_nix_info(package: &str) -> Result<PackageInfo, PackageError> {
    // Try nix search first
    let output = Command::new("nix")
        .args(["search", "nixpkgs", package, "--json"])
        .output()
        .map_err(|e| PackageError::ToolFailed(format!("nix search failed: {}", e)))?;

    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        if let Ok(results) = serde_json::from_str::<serde_json::Value>(&stdout) {
            if let Some(obj) = results.as_object() {
                // Find exact match or first result
                let (attr, info) = obj
                    .iter()
                    .find(|(k, _)| k.ends_with(&format!(".{}", package)))
                    .or_else(|| obj.iter().next())
                    .ok_or_else(|| PackageError::NotFound(package.to_string()))?;

                let name = attr
                    .split('.')
                    .last()
                    .unwrap_or(package)
                    .to_string();

                let version = info
                    .get("version")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown")
                    .to_string();

                let description = info
                    .get("description")
                    .and_then(|d| d.as_str())
                    .map(String::from);

                return Ok(PackageInfo {
                    name,
                    version,
                    description,
                    license: None,
                    homepage: Some(format!("https://search.nixos.org/packages?query={}", package)),
                    repository: None,
                    features: Vec::new(),
                    dependencies: Vec::new(),
                });
            }
        }
    }

    // Fallback: try nix-env
    let output = Command::new("nix-env")
        .args(["-qaP", package])
        .output()
        .map_err(|e| PackageError::ToolFailed(format!("nix-env failed: {}", e)))?;

    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        if let Some(line) = stdout.lines().next() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                // Format: "nixpkgs.package  package-1.2.3"
                let full_name = parts[1];
                let (name, version) = if let Some(idx) = full_name.rfind('-') {
                    let potential_version = &full_name[idx + 1..];
                    if potential_version.chars().next().is_some_and(|c| c.is_ascii_digit()) {
                        (full_name[..idx].to_string(), potential_version.to_string())
                    } else {
                        (full_name.to_string(), "unknown".to_string())
                    }
                } else {
                    (full_name.to_string(), "unknown".to_string())
                };

                return Ok(PackageInfo {
                    name,
                    version,
                    description: None,
                    license: None,
                    homepage: Some(format!("https://search.nixos.org/packages?query={}", package)),
                    repository: None,
                    features: Vec::new(),
                    dependencies: Vec::new(),
                });
            }
        }
    }

    Err(PackageError::NotFound(package.to_string()))
}
