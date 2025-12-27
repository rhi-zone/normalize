//! npm/yarn/pnpm/bun (Node.js) ecosystem.

mod lockfile_bun;
mod lockfile_npm;
mod lockfile_pnpm;
mod lockfile_yarn;

use crate::{
    AuditResult, Dependency, DependencyTree, Ecosystem, LockfileManager, PackageError, PackageInfo,
    PackageQuery, Vulnerability, VulnerabilitySeverity,
};
use std::path::Path;
use std::process::Command;

pub struct Npm;

impl Ecosystem for Npm {
    fn name(&self) -> &'static str {
        "npm"
    }

    fn manifest_files(&self) -> &'static [&'static str] {
        &["package.json"]
    }

    fn lockfiles(&self) -> &'static [LockfileManager] {
        &[
            LockfileManager {
                filename: "pnpm-lock.yaml",
                manager: "pnpm",
            },
            LockfileManager {
                filename: "yarn.lock",
                manager: "yarn",
            },
            LockfileManager {
                filename: "package-lock.json",
                manager: "npm",
            },
            LockfileManager {
                filename: "bun.lock",
                manager: "bun",
            },
            LockfileManager {
                filename: "bun.lockb",
                manager: "bun",
            },
        ]
    }

    fn tools(&self) -> &'static [&'static str] {
        // Fastest first
        &["bun", "pnpm", "yarn", "npm"]
    }

    fn fetch_info(&self, query: &PackageQuery, tool: &str) -> Result<PackageInfo, PackageError> {
        // Format: package or package@version
        let pkg_spec = match &query.version {
            Some(v) => format!("{}@{}", query.name, v),
            None => query.name.clone(),
        };
        match tool {
            "npm" => fetch_npm_info(&query.name, "npm", &["view", &pkg_spec, "--json"]),
            "yarn" => fetch_npm_info(&query.name, "yarn", &["info", &pkg_spec, "--json"]),
            "pnpm" => fetch_npm_info(&query.name, "pnpm", &["view", &pkg_spec, "--json"]),
            "bun" => fetch_npm_info(&query.name, "bun", &["pm", "view", &pkg_spec]),
            _ => Err(PackageError::ToolFailed(format!("unknown tool: {}", tool))),
        }
    }

    fn installed_version(&self, package: &str, project_root: &Path) -> Option<String> {
        // Try each lockfile format
        if let Some(v) = lockfile_npm::installed_version(package, project_root) {
            return Some(v);
        }
        if let Some(v) = lockfile_pnpm::installed_version(package, project_root) {
            return Some(v);
        }
        if let Some(v) = lockfile_yarn::installed_version(package, project_root) {
            return Some(v);
        }
        if let Some(v) = lockfile_bun::installed_version(package, project_root) {
            return Some(v);
        }
        None
    }

    fn list_dependencies(&self, project_root: &Path) -> Result<Vec<Dependency>, PackageError> {
        let manifest = project_root.join("package.json");
        let content = std::fs::read_to_string(&manifest)
            .map_err(|e| PackageError::ParseError(format!("failed to read package.json: {}", e)))?;
        let parsed: serde_json::Value = serde_json::from_str(&content)
            .map_err(|e| PackageError::ParseError(format!("invalid JSON: {}", e)))?;

        let mut deps = Vec::new();

        if let Some(d) = parsed.get("dependencies").and_then(|d| d.as_object()) {
            for (name, version) in d {
                deps.push(Dependency {
                    name: name.clone(),
                    version_req: version.as_str().map(String::from),
                    optional: false,
                });
            }
        }

        if let Some(d) = parsed.get("devDependencies").and_then(|d| d.as_object()) {
            for (name, version) in d {
                deps.push(Dependency {
                    name: name.clone(),
                    version_req: version.as_str().map(String::from),
                    optional: false,
                });
            }
        }

        if let Some(d) = parsed
            .get("optionalDependencies")
            .and_then(|d| d.as_object())
        {
            for (name, version) in d {
                deps.push(Dependency {
                    name: name.clone(),
                    version_req: version.as_str().map(String::from),
                    optional: true,
                });
            }
        }

        Ok(deps)
    }

    fn dependency_tree(&self, project_root: &Path) -> Result<DependencyTree, PackageError> {
        // Try each lockfile format in order of preference
        if let Some(tree) = lockfile_pnpm::dependency_tree(project_root) {
            return tree;
        }
        if let Some(tree) = lockfile_yarn::dependency_tree(project_root) {
            return tree;
        }
        if let Some(tree) = lockfile_npm::dependency_tree(project_root) {
            return tree;
        }
        if let Some(tree) = lockfile_bun::dependency_tree(project_root) {
            return tree;
        }

        Err(PackageError::ParseError(format!(
            "no supported lockfile found in {} or parent directories",
            project_root.display()
        )))
    }

    fn audit(&self, project_root: &Path) -> Result<AuditResult, PackageError> {
        // Try npm audit (built into npm)
        let output = Command::new("npm")
            .args(["audit", "--json"])
            .current_dir(project_root)
            .output()
            .map_err(|e| PackageError::ToolFailed(format!("npm audit failed: {}", e)))?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        if stdout.trim().is_empty() {
            return Ok(AuditResult {
                vulnerabilities: Vec::new(),
            });
        }

        // Parse npm audit JSON output
        let v: serde_json::Value = serde_json::from_str(&stdout)
            .map_err(|e| PackageError::ParseError(format!("invalid JSON: {}", e)))?;

        let mut vulnerabilities = Vec::new();

        // npm audit format has vulnerabilities object with package names as keys
        if let Some(vulns) = v.get("vulnerabilities").and_then(|v| v.as_object()) {
            for (pkg_name, vuln) in vulns {
                let via = vuln.get("via").and_then(|v| v.as_array());
                let version = vuln
                    .get("range")
                    .and_then(|r| r.as_str())
                    .unwrap_or("")
                    .to_string();

                // Each "via" entry is a vulnerability or a dependent package
                if let Some(via_arr) = via {
                    for via_entry in via_arr {
                        // Skip if this is just a package name (string), not a vuln object
                        if via_entry.is_string() {
                            continue;
                        }

                        let title = via_entry
                            .get("title")
                            .and_then(|t| t.as_str())
                            .unwrap_or("Unknown vulnerability")
                            .to_string();
                        let url = via_entry
                            .get("url")
                            .and_then(|u| u.as_str())
                            .map(String::from);
                        let cve = via_entry
                            .get("cwe")
                            .and_then(|c| c.as_array())
                            .and_then(|arr| arr.first())
                            .and_then(|c| c.as_str())
                            .map(String::from);

                        let severity = via_entry
                            .get("severity")
                            .and_then(|s| s.as_str())
                            .map(|s| match s {
                                "critical" => VulnerabilitySeverity::Critical,
                                "high" => VulnerabilitySeverity::High,
                                "moderate" => VulnerabilitySeverity::Medium,
                                "low" => VulnerabilitySeverity::Low,
                                _ => VulnerabilitySeverity::Unknown,
                            })
                            .unwrap_or(VulnerabilitySeverity::Unknown);

                        let fixed_in = vuln.get("fixAvailable").and_then(|f| {
                            if f.is_boolean() {
                                None
                            } else {
                                f.get("version").and_then(|v| v.as_str()).map(String::from)
                            }
                        });

                        vulnerabilities.push(Vulnerability {
                            package: pkg_name.clone(),
                            version: version.clone(),
                            severity,
                            title,
                            url,
                            cve,
                            fixed_in,
                        });
                    }
                }
            }
        }

        Ok(AuditResult { vulnerabilities })
    }
}

fn fetch_npm_info(package: &str, tool: &str, args: &[&str]) -> Result<PackageInfo, PackageError> {
    let output = Command::new(tool)
        .args(args)
        .output()
        .map_err(|e| PackageError::ToolFailed(e.to_string()))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("404") || stderr.contains("not found") {
            return Err(PackageError::NotFound(package.to_string()));
        }
        return Err(PackageError::ToolFailed(stderr.to_string()));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Yarn wraps output in a JSON object with "data" field
    let json_str = if tool == "yarn" {
        extract_yarn_data(&stdout)?
    } else {
        stdout.to_string()
    };

    parse_npm_json(&json_str, package)
}

fn extract_yarn_data(output: &str) -> Result<String, PackageError> {
    // Yarn outputs: {"type":"inspect","data":{...}}
    let parsed: serde_json::Value = serde_json::from_str(output)
        .map_err(|e| PackageError::ParseError(format!("invalid yarn JSON: {}", e)))?;

    if let Some(data) = parsed.get("data") {
        Ok(data.to_string())
    } else {
        // Fallback: maybe it's already the data
        Ok(output.to_string())
    }
}

fn parse_npm_json(json_str: &str, package: &str) -> Result<PackageInfo, PackageError> {
    let v: serde_json::Value = serde_json::from_str(json_str)
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

    let description = v
        .get("description")
        .and_then(|v| v.as_str())
        .map(String::from);

    let license = v.get("license").and_then(|v| v.as_str()).map(String::from);

    let homepage = v.get("homepage").and_then(|v| v.as_str()).map(String::from);

    let repository = v.get("repository").and_then(|r| {
        if let Some(url) = r.as_str() {
            Some(url.to_string())
        } else {
            r.get("url").and_then(|u| u.as_str()).map(String::from)
        }
    });

    // Dependencies
    let mut dependencies = Vec::new();
    if let Some(deps) = v.get("dependencies").and_then(|d| d.as_object()) {
        for (name, version) in deps {
            dependencies.push(Dependency {
                name: name.clone(),
                version_req: version.as_str().map(String::from),
                optional: false,
            });
        }
    }
    if let Some(deps) = v.get("peerDependencies").and_then(|d| d.as_object()) {
        for (name, version) in deps {
            dependencies.push(Dependency {
                name: name.clone(),
                version_req: version.as_str().map(String::from),
                optional: false,
            });
        }
    }
    if let Some(deps) = v.get("optionalDependencies").and_then(|d| d.as_object()) {
        for (name, version) in deps {
            dependencies.push(Dependency {
                name: name.clone(),
                version_req: version.as_str().map(String::from),
                optional: true,
            });
        }
    }

    // npm doesn't have features like Cargo, but we could map optionalDependencies
    let features = Vec::new();

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_npm_json() {
        let json = r#"{
            "name": "react",
            "version": "18.2.0",
            "description": "React is a JavaScript library for building user interfaces.",
            "license": "MIT",
            "homepage": "https://reactjs.org/",
            "repository": {"url": "https://github.com/facebook/react.git"},
            "dependencies": {"loose-envify": "^1.1.0"},
            "peerDependencies": {},
            "optionalDependencies": {}
        }"#;

        let info = parse_npm_json(json, "react").unwrap();
        assert_eq!(info.name, "react");
        assert_eq!(info.version, "18.2.0");
        assert_eq!(info.license, Some("MIT".to_string()));
        assert_eq!(info.dependencies.len(), 1);
        assert_eq!(info.dependencies[0].name, "loose-envify");
    }
}
