//! NuGet (.NET) ecosystem.

use crate::{PackageQuery, Dependency, Ecosystem, LockfileManager, PackageError, PackageInfo};
use std::process::Command;

pub struct Nuget;

impl Ecosystem for Nuget {
    fn name(&self) -> &'static str {
        "nuget"
    }

    fn manifest_files(&self) -> &'static [&'static str] {
        &["*.csproj", "*.fsproj", "*.vbproj", "packages.config"]
    }

    fn lockfiles(&self) -> &'static [LockfileManager] {
        &[LockfileManager {
            filename: "packages.lock.json",
            manager: "dotnet",
        }]
    }

    fn tools(&self) -> &'static [&'static str] {
        &["curl"] // Uses NuGet API
    }

    fn fetch_info(&self, query: &PackageQuery, _tool: &str) -> Result<PackageInfo, PackageError> {
        fetch_nuget_info(&query.name)
    }
}

fn fetch_nuget_info(package: &str) -> Result<PackageInfo, PackageError> {
    // First get the latest version
    let index_url = format!(
        "https://api.nuget.org/v3-flatcontainer/{}/index.json",
        package.to_lowercase()
    );

    let output = Command::new("curl")
        .args(["-sS", "-f", &index_url])
        .output()
        .map_err(|e| PackageError::ToolFailed(format!("curl failed: {}", e)))?;

    if !output.status.success() {
        return Err(PackageError::NotFound(package.to_string()));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let index: serde_json::Value = serde_json::from_str(&stdout)
        .map_err(|e| PackageError::ParseError(format!("invalid JSON: {}", e)))?;

    let version = index
        .get("versions")
        .and_then(|v| v.as_array())
        .and_then(|arr| arr.last())
        .and_then(|v| v.as_str())
        .ok_or_else(|| PackageError::ParseError("no versions found".to_string()))?;

    // Get package metadata from nuspec
    let nuspec_url = format!(
        "https://api.nuget.org/v3-flatcontainer/{}/{}/{}.nuspec",
        package.to_lowercase(),
        version,
        package.to_lowercase()
    );

    let output = Command::new("curl")
        .args(["-sS", "-f", &nuspec_url])
        .output()
        .map_err(|e| PackageError::ToolFailed(format!("curl failed: {}", e)))?;

    if !output.status.success() {
        // Return basic info if nuspec not available
        return Ok(PackageInfo {
            name: package.to_string(),
            version: version.to_string(),
            description: None,
            license: None,
            homepage: Some(format!("https://www.nuget.org/packages/{}", package)),
            repository: None,
            features: Vec::new(),
            dependencies: Vec::new(),
        });
    }

    let nuspec = String::from_utf8_lossy(&output.stdout);
    parse_nuspec(&nuspec, package, version)
}

fn parse_nuspec(xml: &str, package: &str, version: &str) -> Result<PackageInfo, PackageError> {
    // Simple XML parsing - extract key fields
    fn extract_tag(xml: &str, tag: &str) -> Option<String> {
        let start_tag = format!("<{}", tag);
        let end_tag = format!("</{}>", tag);

        let start = xml.find(&start_tag)?;
        let content_start = xml[start..].find('>')? + start + 1;
        let end = xml[content_start..].find(&end_tag)? + content_start;

        let content = xml[content_start..end].trim();
        if content.is_empty() {
            None
        } else {
            Some(content.to_string())
        }
    }

    let description = extract_tag(xml, "description");
    let license = extract_tag(xml, "license").or_else(|| extract_tag(xml, "licenseUrl"));
    let homepage = extract_tag(xml, "projectUrl");
    let repository = extract_tag(xml, "repository");

    // Parse dependencies
    let mut dependencies = Vec::new();
    if let Some(deps_start) = xml.find("<dependencies>") {
        if let Some(deps_end) = xml[deps_start..].find("</dependencies>") {
            let deps_section = &xml[deps_start..deps_start + deps_end];
            // Find all <dependency id="..." version="..." />
            for dep_match in deps_section.split("<dependency") {
                if let Some(id_start) = dep_match.find("id=\"") {
                    let id_content = &dep_match[id_start + 4..];
                    if let Some(id_end) = id_content.find('"') {
                        let dep_name = id_content[..id_end].to_string();
                        let version_req = if let Some(ver_start) = dep_match.find("version=\"") {
                            let ver_content = &dep_match[ver_start + 9..];
                            ver_content.find('"').map(|end| ver_content[..end].to_string())
                        } else {
                            None
                        };
                        dependencies.push(Dependency {
                            name: dep_name,
                            version_req,
                            optional: false,
                        });
                    }
                }
            }
        }
    }

    Ok(PackageInfo {
        name: package.to_string(),
        version: version.to_string(),
        description,
        license,
        homepage,
        repository,
        features: Vec::new(),
        dependencies,
    })
}
