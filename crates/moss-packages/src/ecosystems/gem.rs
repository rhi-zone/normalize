//! RubyGems ecosystem.

use crate::{PackageQuery, Dependency, Ecosystem, LockfileManager, PackageError, PackageInfo};
use std::process::Command;

pub struct Gem;

impl Ecosystem for Gem {
    fn name(&self) -> &'static str {
        "gem"
    }

    fn manifest_files(&self) -> &'static [&'static str] {
        &["Gemfile", "*.gemspec"]
    }

    fn lockfiles(&self) -> &'static [LockfileManager] {
        &[LockfileManager {
            filename: "Gemfile.lock",
            manager: "bundle",
        }]
    }

    fn tools(&self) -> &'static [&'static str] {
        &["curl"] // Uses rubygems.org API
    }

    fn fetch_info(&self, query: &PackageQuery, _tool: &str) -> Result<PackageInfo, PackageError> {
        fetch_rubygems_info(&query.name)
    }
}

fn fetch_rubygems_info(package: &str) -> Result<PackageInfo, PackageError> {
    let url = format!("https://rubygems.org/api/v1/gems/{}.json", package);

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

    let version = v
        .get("version")
        .and_then(|v| v.as_str())
        .ok_or_else(|| PackageError::ParseError("missing version".to_string()))?
        .to_string();

    let description = v.get("info").and_then(|i| i.as_str()).map(String::from);

    let license = v
        .get("licenses")
        .and_then(|l| l.as_array())
        .and_then(|arr| arr.first())
        .and_then(|l| l.as_str())
        .map(String::from);

    let homepage = v
        .get("homepage_uri")
        .and_then(|u| u.as_str())
        .map(String::from);

    let repository = v
        .get("source_code_uri")
        .and_then(|u| u.as_str())
        .map(String::from);

    // Parse dependencies
    let mut dependencies = Vec::new();
    if let Some(deps) = v.get("dependencies") {
        // Runtime dependencies
        if let Some(runtime) = deps.get("runtime").and_then(|r| r.as_array()) {
            for dep in runtime {
                if let Some(dep_name) = dep.get("name").and_then(|n| n.as_str()) {
                    let version_req = dep
                        .get("requirements")
                        .and_then(|r| r.as_str())
                        .map(String::from);
                    dependencies.push(Dependency {
                        name: dep_name.to_string(),
                        version_req,
                        optional: false,
                    });
                }
            }
        }
        // Development dependencies (marked as optional)
        if let Some(dev) = deps.get("development").and_then(|d| d.as_array()) {
            for dep in dev {
                if let Some(dep_name) = dep.get("name").and_then(|n| n.as_str()) {
                    let version_req = dep
                        .get("requirements")
                        .and_then(|r| r.as_str())
                        .map(String::from);
                    dependencies.push(Dependency {
                        name: dep_name.to_string(),
                        version_req,
                        optional: true,
                    });
                }
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
