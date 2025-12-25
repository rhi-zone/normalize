//! Conan (C++) ecosystem.

use crate::{PackageQuery, Ecosystem, LockfileManager, PackageError, PackageInfo};
use std::process::Command;

pub struct Conan;

impl Ecosystem for Conan {
    fn name(&self) -> &'static str {
        "conan"
    }

    fn manifest_files(&self) -> &'static [&'static str] {
        &["conanfile.txt", "conanfile.py"]
    }

    fn lockfiles(&self) -> &'static [LockfileManager] {
        &[LockfileManager {
            filename: "conan.lock",
            manager: "conan",
        }]
    }

    fn tools(&self) -> &'static [&'static str] {
        &["curl"] // Uses ConanCenter GitHub API
    }

    fn fetch_info(&self, query: &PackageQuery, _tool: &str) -> Result<PackageInfo, PackageError> {
        fetch_conancenter_api(&query.name)
    }
}

fn fetch_conancenter_api(package: &str) -> Result<PackageInfo, PackageError> {
    // ConanCenter Web API
    let url = format!(
        "https://raw.githubusercontent.com/conan-io/conan-center-index/master/recipes/{}/config.yml",
        package
    );

    let output = Command::new("curl")
        .args(["-sS", "-f", &url])
        .output()
        .map_err(|e| PackageError::ToolFailed(format!("curl failed: {}", e)))?;

    if !output.status.success() {
        return Err(PackageError::NotFound(package.to_string()));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Parse YAML config - extract versions (format: "1.2.3":)
    let version = stdout
        .lines()
        .find(|line| {
            let t = line.trim().trim_start_matches('"');
            t.chars().next().is_some_and(|c| c.is_ascii_digit())
        })
        .and_then(|line| {
            let trimmed = line.trim().trim_matches(|c| c == '"' || c == ':' || c == ' ');
            if trimmed.chars().next().is_some_and(|c| c.is_ascii_digit()) {
                Some(trimmed.to_string())
            } else {
                None
            }
        })
        .unwrap_or_else(|| "latest".to_string());

    Ok(PackageInfo {
        name: package.to_string(),
        version,
        description: None,
        license: None,
        homepage: Some(format!("https://conan.io/center/recipes/{}", package)),
        repository: Some(format!(
            "https://github.com/conan-io/conan-center-index/tree/master/recipes/{}",
            package
        )),
        features: Vec::new(),
        dependencies: Vec::new(),
    })
}
