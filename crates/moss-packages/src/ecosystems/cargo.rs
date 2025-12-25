//! Cargo (Rust) ecosystem.

use crate::{Ecosystem, Feature, LockfileManager, PackageError, PackageInfo, PackageQuery};
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

    fn fetch_info(&self, query: &PackageQuery, _tool: &str) -> Result<PackageInfo, PackageError> {
        fetch_crates_io_info(query)
    }
}

fn fetch_crates_io_info(query: &PackageQuery) -> Result<PackageInfo, PackageError> {
    let package = &query.name;

    // If version specified, fetch that version directly
    // Otherwise, get crate metadata first to find latest version
    let version = if let Some(v) = &query.version {
        v.clone()
    } else {
        // Get latest version
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

        v.get("crate")
            .and_then(|c| c.get("max_version"))
            .and_then(|v| v.as_str())
            .ok_or_else(|| PackageError::ParseError("missing max_version".to_string()))?
            .to_string()
    };

    // Get version-specific info
    let version_url = format!("https://crates.io/api/v1/crates/{}/{}", package, version);
    let output = Command::new("curl")
        .args(["-sS", "-f", "-H", "User-Agent: moss-packages", &version_url])
        .output()
        .map_err(|e| PackageError::ToolFailed(format!("curl failed: {}", e)))?;

    if !output.status.success() {
        return Err(PackageError::NotFound(format!("{}@{}", package, version)));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let v: serde_json::Value = serde_json::from_str(&stdout)
        .map_err(|e| PackageError::ParseError(format!("invalid JSON: {}", e)))?;

    let ver = v
        .get("version")
        .ok_or_else(|| PackageError::ParseError("missing version".to_string()))?;

    let name = ver
        .get("crate")
        .and_then(|c| c.as_str())
        .unwrap_or(package)
        .to_string();

    let version = ver
        .get("num")
        .and_then(|n| n.as_str())
        .unwrap_or(&version)
        .to_string();

    let license = ver.get("license").and_then(|l| l.as_str()).map(String::from);

    let features = ver
        .get("features")
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

    // Get crate-level info (description, homepage, repository)
    let crate_url = format!("https://crates.io/api/v1/crates/{}", package);
    let crate_output = Command::new("curl")
        .args(["-sS", "-f", "-H", "User-Agent: moss-packages", &crate_url])
        .output()
        .ok();

    let (description, homepage, repository) = if let Some(out) = crate_output {
        if out.status.success() {
            let crate_stdout = String::from_utf8_lossy(&out.stdout);
            if let Ok(cv) = serde_json::from_str::<serde_json::Value>(&crate_stdout) {
                let crate_info = cv.get("crate");
                (
                    crate_info
                        .and_then(|c| c.get("description"))
                        .and_then(|d| d.as_str())
                        .map(String::from),
                    crate_info
                        .and_then(|c| c.get("homepage"))
                        .and_then(|h| h.as_str())
                        .filter(|s| !s.is_empty())
                        .map(String::from),
                    crate_info
                        .and_then(|c| c.get("repository"))
                        .and_then(|r| r.as_str())
                        .filter(|s| !s.is_empty())
                        .map(String::from),
                )
            } else {
                (None, None, None)
            }
        } else {
            (None, None, None)
        }
    } else {
        (None, None, None)
    };

    Ok(PackageInfo {
        name,
        version,
        description,
        license,
        homepage,
        repository,
        features,
        dependencies: Vec::new(),
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
