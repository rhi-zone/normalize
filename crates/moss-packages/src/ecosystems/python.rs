//! Python (pip/uv/poetry) ecosystem.

use crate::{Dependency, Ecosystem, Feature, LockfileManager, PackageError, PackageInfo, PackageQuery};
use std::path::Path;
use std::process::Command;

pub struct Python;

impl Ecosystem for Python {
    fn name(&self) -> &'static str {
        "python"
    }

    fn manifest_files(&self) -> &'static [&'static str] {
        &["pyproject.toml", "setup.py", "requirements.txt"]
    }

    fn lockfiles(&self) -> &'static [LockfileManager] {
        &[
            LockfileManager {
                filename: "uv.lock",
                manager: "uv",
            },
            LockfileManager {
                filename: "poetry.lock",
                manager: "poetry",
            },
            LockfileManager {
                filename: "Pipfile.lock",
                manager: "pipenv",
            },
            LockfileManager {
                filename: "pdm.lock",
                manager: "pdm",
            },
        ]
    }

    fn tools(&self) -> &'static [&'static str] {
        &["curl"] // Uses PyPI API
    }

    fn fetch_info(&self, query: &PackageQuery, _tool: &str) -> Result<PackageInfo, PackageError> {
        fetch_pypi_info(query)
    }

    fn installed_version(&self, package: &str, project_root: &Path) -> Option<String> {
        // Normalize package name (PEP 503: lowercase, replace - and . with _)
        let normalized = package.to_lowercase().replace(['-', '.'], "_");

        // Try uv.lock (TOML format)
        let uv_lock = project_root.join("uv.lock");
        if let Ok(content) = std::fs::read_to_string(&uv_lock) {
            if let Ok(parsed) = toml::from_str::<toml::Value>(&content) {
                if let Some(packages) = parsed.get("package").and_then(|p| p.as_array()) {
                    for pkg in packages {
                        let name = pkg.get("name").and_then(|n| n.as_str()).unwrap_or("");
                        let name_normalized = name.to_lowercase().replace(['-', '.'], "_");
                        if name_normalized == normalized {
                            if let Some(v) = pkg.get("version").and_then(|v| v.as_str()) {
                                return Some(v.to_string());
                            }
                        }
                    }
                }
            }
        }

        // Try poetry.lock (TOML format)
        let poetry_lock = project_root.join("poetry.lock");
        if let Ok(content) = std::fs::read_to_string(&poetry_lock) {
            if let Ok(parsed) = toml::from_str::<toml::Value>(&content) {
                if let Some(packages) = parsed.get("package").and_then(|p| p.as_array()) {
                    for pkg in packages {
                        let name = pkg.get("name").and_then(|n| n.as_str()).unwrap_or("");
                        let name_normalized = name.to_lowercase().replace(['-', '.'], "_");
                        if name_normalized == normalized {
                            if let Some(v) = pkg.get("version").and_then(|v| v.as_str()) {
                                return Some(v.to_string());
                            }
                        }
                    }
                }
            }
        }

        // Try Pipfile.lock (JSON format)
        let pipfile_lock = project_root.join("Pipfile.lock");
        if let Ok(content) = std::fs::read_to_string(pipfile_lock) {
            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&content) {
                // Check default and develop sections
                for section in ["default", "develop"] {
                    if let Some(deps) = parsed.get(section).and_then(|s| s.as_object()) {
                        for (name, info) in deps {
                            let name_normalized = name.to_lowercase().replace(['-', '.'], "_");
                            if name_normalized == normalized {
                                if let Some(v) = info.get("version").and_then(|v| v.as_str()) {
                                    // Strip "==" prefix
                                    return Some(v.strip_prefix("==").unwrap_or(v).to_string());
                                }
                            }
                        }
                    }
                }
            }
        }

        None
    }
}

fn fetch_pypi_info(query: &PackageQuery) -> Result<PackageInfo, PackageError> {
    // PyPI API: /pypi/{package}/json for latest, /pypi/{package}/{version}/json for specific
    let url = match &query.version {
        Some(v) => format!("https://pypi.org/pypi/{}/{}/json", query.name, v),
        None => format!("https://pypi.org/pypi/{}/json", query.name),
    };

    let output = Command::new("curl")
        .args(["-sS", "-f", &url])
        .output()
        .map_err(|e| PackageError::ToolFailed(format!("curl failed: {}", e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("404") || output.status.code() == Some(22) {
            return Err(PackageError::NotFound(query.name.clone()));
        }
        return Err(PackageError::RegistryError(stderr.to_string()));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_pypi_json(&stdout, &query.name)
}

fn parse_pypi_json(json_str: &str, package: &str) -> Result<PackageInfo, PackageError> {
    let v: serde_json::Value = serde_json::from_str(json_str)
        .map_err(|e| PackageError::ParseError(format!("invalid JSON: {}", e)))?;

    let info = v
        .get("info")
        .ok_or_else(|| PackageError::ParseError("missing info field".to_string()))?;

    let name = info
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or(package)
        .to_string();

    let version = info
        .get("version")
        .and_then(|v| v.as_str())
        .ok_or_else(|| PackageError::ParseError("missing version".to_string()))?
        .to_string();

    let description = info
        .get("summary")
        .and_then(|v| v.as_str())
        .map(String::from);

    let license = info
        .get("license")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(String::from);

    let homepage = info
        .get("home_page")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(String::from);

    let repository = info
        .get("project_urls")
        .and_then(|urls| {
            urls.get("Source")
                .or_else(|| urls.get("Repository"))
                .or_else(|| urls.get("GitHub"))
                .and_then(|v| v.as_str())
                .map(String::from)
        });

    // Parse requires_dist for dependencies
    let mut dependencies = Vec::new();
    if let Some(requires) = info.get("requires_dist").and_then(|r| r.as_array()) {
        for req in requires {
            if let Some(req_str) = req.as_str() {
                if let Some(dep) = parse_requirement(req_str) {
                    dependencies.push(dep);
                }
            }
        }
    }

    // Parse extras as features
    let mut features = Vec::new();
    if let Some(extras) = info.get("provides_extra").and_then(|e| e.as_array()) {
        for extra in extras {
            if let Some(extra_name) = extra.as_str() {
                // Find dependencies that require this extra
                let extra_deps: Vec<String> = dependencies
                    .iter()
                    .filter(|d| d.version_req.as_ref().is_some_and(|v| v.contains(&format!("extra == '{}'", extra_name))))
                    .map(|d| d.name.clone())
                    .collect();

                features.push(Feature {
                    name: extra_name.to_string(),
                    description: None,
                    dependencies: extra_deps,
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
        features,
        dependencies,
    })
}

fn parse_requirement(req: &str) -> Option<Dependency> {
    // Parse PEP 508 requirement: "name[extra] (>=1.0) ; marker"
    let req = req.trim();

    // Split on ; to separate requirement from marker
    let (req_part, marker) = req.split_once(';').map(|(a, b)| (a.trim(), Some(b))).unwrap_or((req, None));

    // Find the package name (before any [, (, <, >, =, !)
    let name_end = req_part
        .find(|c: char| c == '[' || c == '(' || c == ' ' || c == '<' || c == '>' || c == '=' || c == '!')
        .unwrap_or(req_part.len());

    let name = req_part[..name_end].trim().to_string();
    if name.is_empty() {
        return None;
    }

    // Extract version requirement (only from the part before the marker)
    let version_req = if let Some(start) = req_part.find(|c: char| c == '<' || c == '>' || c == '=' || c == '!') {
        let version_part = req_part[start..].trim();
        if version_part.is_empty() {
            None
        } else {
            Some(version_part.to_string())
        }
    } else {
        None
    };

    // Check if optional (has marker with "extra")
    let optional = marker.is_some_and(|m| m.contains("extra"));

    Some(Dependency {
        name,
        version_req,
        optional,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_requirement() {
        let dep = parse_requirement("requests>=2.0").unwrap();
        assert_eq!(dep.name, "requests");
        assert_eq!(dep.version_req, Some(">=2.0".to_string()));
        assert!(!dep.optional);

        let dep = parse_requirement("pytest ; extra == 'dev'").unwrap();
        assert_eq!(dep.name, "pytest");
        assert!(dep.optional);

        let dep = parse_requirement("numpy").unwrap();
        assert_eq!(dep.name, "numpy");
        assert_eq!(dep.version_req, None);
    }

    #[test]
    fn test_parse_pypi_json() {
        let json = r#"{
            "info": {
                "name": "requests",
                "version": "2.32.0",
                "summary": "Python HTTP for Humans.",
                "license": "Apache-2.0",
                "home_page": "https://requests.readthedocs.io",
                "project_urls": {
                    "Source": "https://github.com/psf/requests"
                },
                "requires_dist": [
                    "charset-normalizer>=2,<4",
                    "idna>=2.5,<4"
                ],
                "provides_extra": ["socks"]
            }
        }"#;

        let info = parse_pypi_json(json, "requests").unwrap();
        assert_eq!(info.name, "requests");
        assert_eq!(info.version, "2.32.0");
        assert_eq!(info.license, Some("Apache-2.0".to_string()));
        assert_eq!(info.dependencies.len(), 2);
    }
}
