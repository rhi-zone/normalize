//! Cargo (Rust) ecosystem.

use crate::{Ecosystem, Feature, LockfileManager, PackageError, PackageInfo};
use std::process::Command;

pub struct Cargo;

impl Ecosystem for Cargo {
    fn name(&self) -> &'static str {
        "cargo"
    }

    fn manifest_files(&self) -> &'static [&'static str] {
        &["Cargo.toml"]
    }

    fn lockfiles(&self) -> &'static [LockfileManager] {
        &[LockfileManager {
            filename: "Cargo.lock",
            manager: "cargo",
        }]
    }

    fn tools(&self) -> &'static [&'static str] {
        &["curl"] // Uses crates.io API
    }

    fn fetch_info(&self, package: &str, _tool: &str) -> Result<PackageInfo, PackageError> {
        fetch_crates_io_info(package)
    }
}

fn fetch_crates_io_info(package: &str) -> Result<PackageInfo, PackageError> {
    // Get crate metadata
    let url = format!("https://crates.io/api/v1/crates/{}", package);

    let output = Command::new("curl")
        .args(["-sS", "-f", "-H", "User-Agent: moss-packages", &url])
        .output()
        .map_err(|e| PackageError::ToolFailed(format!("curl failed: {}", e)))?;

    if !output.status.success() {
        return Err(PackageError::NotFound(package.to_string()));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let v: serde_json::Value = serde_json::from_str(&stdout)
        .map_err(|e| PackageError::ParseError(format!("invalid JSON: {}", e)))?;

    let crate_info = v
        .get("crate")
        .ok_or_else(|| PackageError::ParseError("missing crate field".to_string()))?;

    let name = crate_info
        .get("name")
        .and_then(|n| n.as_str())
        .unwrap_or(package)
        .to_string();

    let version = crate_info
        .get("max_version")
        .and_then(|v| v.as_str())
        .ok_or_else(|| PackageError::ParseError("missing max_version".to_string()))?
        .to_string();

    let description = crate_info
        .get("description")
        .and_then(|d| d.as_str())
        .map(String::from);

    let homepage = crate_info
        .get("homepage")
        .and_then(|h| h.as_str())
        .filter(|s| !s.is_empty())
        .map(String::from);

    let repository = crate_info
        .get("repository")
        .and_then(|r| r.as_str())
        .filter(|s| !s.is_empty())
        .map(String::from);

    // Get version-specific info (license, features)
    let version_url = format!("https://crates.io/api/v1/crates/{}/{}", package, version);

    let version_output = Command::new("curl")
        .args(["-sS", "-f", "-H", "User-Agent: moss-packages", &version_url])
        .output()
        .map_err(|e| PackageError::ToolFailed(format!("curl failed: {}", e)))?;

    let (license, features) = if version_output.status.success() {
        let version_stdout = String::from_utf8_lossy(&version_output.stdout);
        if let Ok(vv) = serde_json::from_str::<serde_json::Value>(&version_stdout) {
            let ver = vv.get("version");

            let lic = ver
                .and_then(|v| v.get("license"))
                .and_then(|l| l.as_str())
                .map(String::from);

            let feats = ver
                .and_then(|v| v.get("features"))
                .and_then(|f| f.as_object())
                .map(|obj| {
                    obj.iter()
                        .map(|(name, deps)| Feature {
                            name: name.clone(),
                            description: None,
                            dependencies: deps
                                .as_array()
                                .map(|arr| {
                                    arr.iter()
                                        .filter_map(|d| d.as_str().map(String::from))
                                        .collect()
                                })
                                .unwrap_or_default(),
                        })
                        .collect()
                })
                .unwrap_or_default();

            (lic, feats)
        } else {
            (None, Vec::new())
        }
    } else {
        (None, Vec::new())
    };

    Ok(PackageInfo {
        name,
        version,
        description,
        license,
        homepage,
        repository,
        features,
        dependencies: Vec::new(), // Would need separate API call
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cargo_ecosystem() {
        let eco = Cargo;
        assert_eq!(eco.name(), "cargo");
        assert_eq!(eco.manifest_files(), &["Cargo.toml"]);
    }
}
