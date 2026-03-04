//! Parser for `vcpkg.json` files (Microsoft vcpkg C/C++ package manager).

use crate::{DeclaredDep, DepKind, ManifestError, ManifestParser, ParsedManifest};
use serde_json::Value;

/// Parser for `vcpkg.json` files.
///
/// Dependencies can be plain strings or objects with a `"name"` key.
/// `"dev-dependencies"` entries are tagged as `DepKind::Dev`.
pub struct VcpkgParser;

impl ManifestParser for VcpkgParser {
    fn filename(&self) -> &'static str {
        "vcpkg.json"
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
            .or_else(|| json.get("version-string"))
            .or_else(|| json.get("version-semver"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let mut deps = Vec::new();
        extract_vcpkg_deps(&json, "dependencies", DepKind::Normal, &mut deps);
        extract_vcpkg_deps(&json, "dev-dependencies", DepKind::Dev, &mut deps);

        Ok(ParsedManifest {
            ecosystem: "vcpkg",
            name,
            version,
            dependencies: deps,
        })
    }
}

fn extract_vcpkg_deps(json: &Value, field: &str, kind: DepKind, out: &mut Vec<DeclaredDep>) {
    let Some(arr) = json.get(field).and_then(|v| v.as_array()) else {
        return;
    };
    for entry in arr {
        let dep_name = if let Some(s) = entry.as_str() {
            s.to_string()
        } else if let Some(obj) = entry.as_object() {
            let Some(n) = obj.get("name").and_then(|v| v.as_str()) else {
                continue;
            };
            n.to_string()
        } else {
            continue;
        };

        // Version constraint lives under "version>=" or similar keys
        let version_req = entry
            .as_object()
            .and_then(|obj| {
                obj.iter()
                    .find(|(k, _)| k.starts_with("version"))
                    .and_then(|(_, v)| v.as_str())
            })
            .map(|s| s.to_string());

        out.push(DeclaredDep {
            name: dep_name,
            version_req,
            kind,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ManifestParser;

    #[test]
    fn test_vcpkg_json() {
        let content = r#"{
  "name": "my-project",
  "version": "1.0",
  "dependencies": [
    "boost",
    { "name": "zlib", "version>=": "1.2.11" },
    { "name": "qt5", "features": ["gui"] }
  ],
  "dev-dependencies": [
    "catch2"
  ]
}"#;
        let m = VcpkgParser.parse(content).unwrap();
        assert_eq!(m.ecosystem, "vcpkg");
        assert_eq!(m.name.as_deref(), Some("my-project"));
        assert_eq!(m.version.as_deref(), Some("1.0"));
        assert_eq!(m.dependencies.len(), 4);

        let boost = m.dependencies.iter().find(|d| d.name == "boost").unwrap();
        assert_eq!(boost.kind, DepKind::Normal);
        assert!(boost.version_req.is_none());

        let zlib = m.dependencies.iter().find(|d| d.name == "zlib").unwrap();
        assert_eq!(zlib.kind, DepKind::Normal);
        assert_eq!(zlib.version_req.as_deref(), Some("1.2.11"));

        let qt5 = m.dependencies.iter().find(|d| d.name == "qt5").unwrap();
        assert_eq!(qt5.kind, DepKind::Normal);
        assert!(qt5.version_req.is_none());

        let catch2 = m.dependencies.iter().find(|d| d.name == "catch2").unwrap();
        assert_eq!(catch2.kind, DepKind::Dev);
    }

    #[test]
    fn test_vcpkg_string_only_deps() {
        let content = r#"{"name": "simple", "dependencies": ["openssl", "curl"]}"#;
        let m = VcpkgParser.parse(content).unwrap();
        assert_eq!(m.dependencies.len(), 2);
        assert!(m.dependencies.iter().all(|d| d.kind == DepKind::Normal));
    }
}
