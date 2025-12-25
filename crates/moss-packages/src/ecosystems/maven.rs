//! Maven (Java) ecosystem.

use crate::{PackageQuery, Ecosystem, LockfileManager, PackageError, PackageInfo};
use std::path::Path;
use std::process::Command;

pub struct Maven;

impl Ecosystem for Maven {
    fn name(&self) -> &'static str {
        "maven"
    }

    fn manifest_files(&self) -> &'static [&'static str] {
        &["pom.xml", "build.gradle", "build.gradle.kts"]
    }

    fn lockfiles(&self) -> &'static [LockfileManager] {
        &[
            LockfileManager {
                filename: "gradle.lockfile",
                manager: "gradle",
            },
            LockfileManager {
                filename: "buildscript-gradle.lockfile",
                manager: "gradle",
            },
        ]
    }

    fn tools(&self) -> &'static [&'static str] {
        &["curl"] // Uses Maven Central API
    }

    fn fetch_info(&self, query: &PackageQuery, _tool: &str) -> Result<PackageInfo, PackageError> {
        fetch_maven_info(&query.name)
    }

    fn installed_version(&self, package: &str, project_root: &Path) -> Option<String> {
        // gradle.lockfile format:
        // group:artifact:version=hash
        let lockfile = project_root.join("gradle.lockfile");
        let content = std::fs::read_to_string(lockfile).ok()?;

        // Package can be "group:artifact" or just "artifact"
        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            // Format: group:artifact:version=hash
            let coord = line.split('=').next()?;
            let parts: Vec<&str> = coord.split(':').collect();
            if parts.len() >= 3 {
                let coord_str = format!("{}:{}", parts[0], parts[1]);
                if coord_str == package || parts[1] == package {
                    return Some(parts[2].to_string());
                }
            }
        }
        None
    }
}

fn fetch_maven_info(package: &str) -> Result<PackageInfo, PackageError> {
    // Package format: groupId:artifactId or groupId:artifactId:version
    let parts: Vec<&str> = package.split(':').collect();
    let (group_id, artifact_id) = match parts.len() {
        1 => {
            // Try to find in Maven Central search
            return search_maven_central(package);
        }
        2 => (parts[0], parts[1]),
        _ => (parts[0], parts[1]),
    };

    // Query Maven Central API
    let url = format!(
        "https://search.maven.org/solrsearch/select?q=g:{}+AND+a:{}&rows=1&wt=json",
        group_id, artifact_id
    );

    let output = Command::new("curl")
        .args(["-sS", "-f", &url])
        .output()
        .map_err(|e| PackageError::ToolFailed(format!("curl failed: {}", e)))?;

    if !output.status.success() {
        return Err(PackageError::NotFound(package.to_string()));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_maven_response(&stdout, package)
}

fn search_maven_central(query: &str) -> Result<PackageInfo, PackageError> {
    let url = format!(
        "https://search.maven.org/solrsearch/select?q={}&rows=1&wt=json",
        query
    );

    let output = Command::new("curl")
        .args(["-sS", "-f", &url])
        .output()
        .map_err(|e| PackageError::ToolFailed(format!("curl failed: {}", e)))?;

    if !output.status.success() {
        return Err(PackageError::NotFound(query.to_string()));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_maven_response(&stdout, query)
}

fn parse_maven_response(json: &str, package: &str) -> Result<PackageInfo, PackageError> {
    let v: serde_json::Value = serde_json::from_str(json)
        .map_err(|e| PackageError::ParseError(format!("invalid JSON: {}", e)))?;

    let docs = v
        .get("response")
        .and_then(|r| r.get("docs"))
        .and_then(|d| d.as_array())
        .ok_or_else(|| PackageError::ParseError("missing response.docs".to_string()))?;

    let doc = docs
        .first()
        .ok_or_else(|| PackageError::NotFound(package.to_string()))?;

    let group_id = doc.get("g").and_then(|g| g.as_str()).unwrap_or("");
    let artifact_id = doc.get("a").and_then(|a| a.as_str()).unwrap_or(package);

    let name = format!("{}:{}", group_id, artifact_id);

    let version = doc
        .get("latestVersion")
        .and_then(|v| v.as_str())
        .ok_or_else(|| PackageError::ParseError("missing latestVersion".to_string()))?
        .to_string();

    // Maven Central search doesn't provide much metadata
    // Would need to fetch pom.xml for full info
    Ok(PackageInfo {
        name,
        version,
        description: None,
        license: None,
        homepage: Some(format!(
            "https://central.sonatype.com/artifact/{}/{}",
            group_id, artifact_id
        )),
        repository: None,
        features: Vec::new(),
        dependencies: Vec::new(),
    })
}
