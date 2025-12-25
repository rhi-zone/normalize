//! Hex (Elixir/Erlang) ecosystem.

use crate::{PackageQuery, Dependency, Ecosystem, LockfileManager, PackageError, PackageInfo};
use std::path::Path;
use std::process::Command;

pub struct Hex;

impl Ecosystem for Hex {
    fn name(&self) -> &'static str {
        "hex"
    }

    fn manifest_files(&self) -> &'static [&'static str] {
        &["mix.exs"]
    }

    fn lockfiles(&self) -> &'static [LockfileManager] {
        &[LockfileManager {
            filename: "mix.lock",
            manager: "mix",
        }]
    }

    fn tools(&self) -> &'static [&'static str] {
        &["curl"] // Uses hex.pm API
    }

    fn fetch_info(&self, query: &PackageQuery, _tool: &str) -> Result<PackageInfo, PackageError> {
        fetch_hex_info(&query.name)
    }

    fn installed_version(&self, package: &str, project_root: &Path) -> Option<String> {
        // mix.lock is Elixir term format:
        //   "package_name": {:hex, :package_name, "1.2.3", ...}
        let lockfile = project_root.join("mix.lock");
        let content = std::fs::read_to_string(lockfile).ok()?;

        // Simple text extraction: look for "package": {:hex, :package, "version"
        let pattern = format!("\"{}\": {{:hex, :{}, \"", package, package);
        if let Some(start) = content.find(&pattern) {
            let after = &content[start + pattern.len()..];
            if let Some(end) = after.find('"') {
                return Some(after[..end].to_string());
            }
        }
        None
    }
}

fn fetch_hex_info(package: &str) -> Result<PackageInfo, PackageError> {
    let url = format!("https://hex.pm/api/packages/{}", package);

    let output = Command::new("curl")
        .args(["-sS", "-f", &url])
        .output()
        .map_err(|e| PackageError::ToolFailed(format!("curl failed: {}", e)))?;

    if !output.status.success() {
        return Err(PackageError::NotFound(package.to_string()));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let v: serde_json::Value = serde_json::from_str(&stdout)
        .map_err(|e| PackageError::ParseError(format!("invalid JSON: {}", e)))?;

    let name = v
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or(package)
        .to_string();

    // Get latest version from releases array
    let version = v
        .get("releases")
        .and_then(|r| r.as_array())
        .and_then(|arr| arr.first())
        .and_then(|r| r.get("version"))
        .and_then(|v| v.as_str())
        .ok_or_else(|| PackageError::ParseError("missing version".to_string()))?
        .to_string();

    let meta = v.get("meta");

    let description = meta
        .and_then(|m| m.get("description"))
        .and_then(|d| d.as_str())
        .map(String::from);

    let license = meta
        .and_then(|m| m.get("licenses"))
        .and_then(|l| l.as_array())
        .and_then(|arr| arr.first())
        .and_then(|l| l.as_str())
        .map(String::from);

    let homepage = meta
        .and_then(|m| m.get("links"))
        .and_then(|l| l.get("GitHub").or(l.get("Homepage")))
        .and_then(|u| u.as_str())
        .map(String::from);

    let repository = meta
        .and_then(|m| m.get("links"))
        .and_then(|l| l.get("GitHub"))
        .and_then(|u| u.as_str())
        .map(String::from);

    // Parse requirements from latest release
    let mut dependencies = Vec::new();
    if let Some(latest) = v.get("releases").and_then(|r| r.as_array()).and_then(|a| a.first()) {
        if let Some(reqs) = latest.get("requirements").and_then(|r| r.as_object()) {
            for (dep_name, req) in reqs {
                let version_req = req
                    .get("requirement")
                    .and_then(|r| r.as_str())
                    .map(String::from);
                let optional = req
                    .get("optional")
                    .and_then(|o| o.as_bool())
                    .unwrap_or(false);
                dependencies.push(Dependency {
                    name: dep_name.clone(),
                    version_req,
                    optional,
                });
            }
        }
    }

    Ok(PackageInfo {
        name,
        version,
        description,
        license,
        homepage,
        repository,
        features: Vec::new(),
        dependencies,
    })
}
