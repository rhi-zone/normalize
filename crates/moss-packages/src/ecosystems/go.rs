//! Go modules ecosystem.

use crate::{PackageQuery, Ecosystem, LockfileManager, PackageError, PackageInfo};
use std::process::Command;

pub struct Go;

impl Ecosystem for Go {
    fn name(&self) -> &'static str {
        "go"
    }

    fn manifest_files(&self) -> &'static [&'static str] {
        &["go.mod"]
    }

    fn lockfiles(&self) -> &'static [LockfileManager] {
        &[LockfileManager {
            filename: "go.sum",
            manager: "go",
        }]
    }

    fn tools(&self) -> &'static [&'static str] {
        &["curl"] // Uses Go module proxy API
    }

    fn fetch_info(&self, query: &PackageQuery, _tool: &str) -> Result<PackageInfo, PackageError> {
        fetch_go_proxy_info(&query.name)
    }
}

fn fetch_go_proxy_info(package: &str) -> Result<PackageInfo, PackageError> {
    // Get latest version from proxy
    let url = format!("https://proxy.golang.org/{}/@latest", package);

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

    let version = v
        .get("Version")
        .and_then(|v| v.as_str())
        .ok_or_else(|| PackageError::ParseError("missing Version".to_string()))?
        .to_string();

    // Try to get more info from pkg.go.dev (optional, may fail)
    let repository = if package.starts_with("github.com/") {
        Some(format!("https://{}", package))
    } else if package.starts_with("golang.org/x/") {
        Some(format!(
            "https://go.googlesource.com/{}",
            package.strip_prefix("golang.org/x/").unwrap()
        ))
    } else {
        None
    };

    Ok(PackageInfo {
        name: package.to_string(),
        version,
        description: None, // Go proxy doesn't provide description
        license: None,
        homepage: Some(format!("https://pkg.go.dev/{}", package)),
        repository,
        features: Vec::new(),
        dependencies: Vec::new(), // Would need to parse go.mod
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_go_ecosystem() {
        let eco = Go;
        assert_eq!(eco.name(), "go");
        assert_eq!(eco.manifest_files(), &["go.mod"]);
    }
}
