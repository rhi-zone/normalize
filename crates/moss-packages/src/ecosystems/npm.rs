//! npm/yarn/pnpm (Node.js) ecosystem.

use crate::{Dependency, Ecosystem, LockfileManager, PackageError, PackageInfo, PackageQuery};
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

    let description = v.get("description").and_then(|v| v.as_str()).map(String::from);

    let license = v.get("license").and_then(|v| v.as_str()).map(String::from);

    let homepage = v.get("homepage").and_then(|v| v.as_str()).map(String::from);

    let repository = v
        .get("repository")
        .and_then(|r| {
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
