//! Parsers for Dub package manifests (D language).
//!
//! - `dub.json` — JSON format (most common)
//! - `dub.sdl` — custom SDL format

use crate::{DeclaredDep, DepKind, ManifestError, ManifestParser, ParsedManifest};
use serde_json::Value;

/// Parser for `dub.json` files.
pub struct DubJsonParser;

impl ManifestParser for DubJsonParser {
    fn filename(&self) -> &'static str {
        "dub.json"
    }

    fn parse(&self, content: &str) -> Result<ParsedManifest, ManifestError> {
        let json: Value =
            serde_json::from_str(content).map_err(|e| ManifestError(e.to_string()))?;

        let name = json
            .get("name")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let version = json
            .get("version")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let mut deps = Vec::new();
        extract_dub_deps(&json, "dependencies", DepKind::Normal, &mut deps);
        extract_dub_deps(&json, "devDependencies", DepKind::Dev, &mut deps);
        extract_dub_deps(&json, "optionalDependencies", DepKind::Optional, &mut deps);

        Ok(ParsedManifest {
            ecosystem: "dub",
            name,
            version,
            dependencies: deps,
        })
    }
}

fn extract_dub_deps(json: &Value, field: &str, kind: DepKind, out: &mut Vec<DeclaredDep>) {
    let Some(obj) = json.get(field) else { return };

    if let Some(map) = obj.as_object() {
        for (name, ver) in map {
            let version_req = if let Some(s) = ver.as_str() {
                Some(s.to_string())
            } else if let Some(obj) = ver.as_object() {
                obj.get("version")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
            } else {
                None
            };
            out.push(DeclaredDep {
                name: name.clone(),
                version_req,
                kind,
            });
        }
    }
}

/// Parser for `dub.sdl` files (SDL = Simple Declarative Language).
pub struct DubSdlParser;

impl ManifestParser for DubSdlParser {
    fn filename(&self) -> &'static str {
        "dub.sdl"
    }

    fn parse(&self, content: &str) -> Result<ParsedManifest, ManifestError> {
        let mut name = None;
        let mut version = None;
        let mut deps = Vec::new();

        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with("//") || trimmed.starts_with('#') {
                continue;
            }

            // name "mypkg"
            if trimmed.starts_with("name ") && name.is_none() {
                name = extract_sdl_string(trimmed);
                continue;
            }
            // version "1.0.0"
            if trimmed.starts_with("version ") && version.is_none() {
                version = extract_sdl_string(trimmed);
                continue;
            }

            // dependency "pkgname" version=">=1.0.0"
            // dependency "pkgname" version="~>1.0"
            if trimmed.starts_with("dependency ")
                && let Some(dep) = parse_sdl_dep(trimmed)
            {
                deps.push(dep);
            }
        }

        Ok(ParsedManifest {
            ecosystem: "dub",
            name,
            version,
            dependencies: deps,
        })
    }
}

fn extract_sdl_string(line: &str) -> Option<String> {
    let start = line.find('"')? + 1;
    let end = line[start..].find('"')?;
    Some(line[start..start + end].to_string())
}

fn parse_sdl_dep(line: &str) -> Option<DeclaredDep> {
    // dependency "pkgname" version=">=1.0.0" optional=true
    let pkg_name = extract_sdl_string(line)?;

    // version="..."
    let version_req = if let Some(ver_start) = line.find("version=\"") {
        let rest = &line[ver_start + 9..];
        rest.find('"').map(|end| rest[..end].to_string())
    } else {
        None
    };

    let kind = if line.contains("optional=true") {
        DepKind::Optional
    } else {
        DepKind::Normal
    };

    Some(DeclaredDep {
        name: pkg_name,
        version_req,
        kind,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ManifestParser;

    #[test]
    fn test_dub_json() {
        let content = r#"{
  "name": "my-d-project",
  "version": "0.1.0",
  "dependencies": {
    "vibe-d": "~>0.9",
    "mir-algorithm": {"version": ">=3.10.0"}
  }
}"#;
        let m = DubJsonParser.parse(content).unwrap();
        assert_eq!(m.ecosystem, "dub");
        assert_eq!(m.name.as_deref(), Some("my-d-project"));
        assert_eq!(m.version.as_deref(), Some("0.1.0"));
        assert_eq!(m.dependencies.len(), 2);

        let vibe = m.dependencies.iter().find(|d| d.name == "vibe-d").unwrap();
        assert_eq!(vibe.version_req.as_deref(), Some("~>0.9"));
    }

    #[test]
    fn test_dub_sdl() {
        let content = r#"name "my-d-project"
version "0.1.0"
dependency "vibe-d" version="~>0.9"
dependency "mir-algorithm" version=">=3.10.0"
"#;
        let m = DubSdlParser.parse(content).unwrap();
        assert_eq!(m.ecosystem, "dub");
        assert_eq!(m.name.as_deref(), Some("my-d-project"));
        assert_eq!(m.dependencies.len(), 2);

        let mir = m
            .dependencies
            .iter()
            .find(|d| d.name == "mir-algorithm")
            .unwrap();
        assert_eq!(mir.version_req.as_deref(), Some(">=3.10.0"));
    }
}
