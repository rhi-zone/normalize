//! Parser for `package.json` files (npm/Node.js).

use crate::{DeclaredDep, DepKind, ManifestError, ManifestParser, ParsedManifest};
use serde_json::Value;

/// Parser for `package.json` files.
pub struct NpmParser;

impl ManifestParser for NpmParser {
    fn filename(&self) -> &'static str {
        "package.json"
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
        extract_npm_deps(&json, "dependencies", DepKind::Normal, &mut deps);
        extract_npm_deps(&json, "devDependencies", DepKind::Dev, &mut deps);
        extract_npm_deps(&json, "peerDependencies", DepKind::Optional, &mut deps);

        Ok(ParsedManifest {
            ecosystem: "npm",
            name,
            version,
            dependencies: deps,
        })
    }
}

fn extract_npm_deps(json: &Value, field: &str, kind: DepKind, out: &mut Vec<DeclaredDep>) {
    if let Some(obj) = json.get(field).and_then(|v| v.as_object()) {
        for (name, ver) in obj {
            out.push(DeclaredDep {
                name: name.clone(),
                version_req: ver.as_str().map(|s| s.to_string()),
                kind,
            });
        }
    }
}

/// Extract the entry point path from `package.json` content.
///
/// Checks `exports`, `module`, and `main` fields in order and returns the
/// raw relative path string (e.g., `"./dist/index.js"`).  Existence checking
/// is left to the caller.
///
/// This replaces the ad-hoc `get_package_entry_point` / `find_package_entry`
/// functions in `normalize-local-deps`.
pub fn npm_entry_point(content: &str) -> Option<String> {
    let json: Value = serde_json::from_str(content).ok()?;

    // exports field
    if let Some(exports) = json.get("exports") {
        if let Some(s) = exports.as_str() {
            return Some(s.to_string());
        }
        if let Some(obj) = exports.as_object()
            && let Some(dot) = obj.get(".")
            && let Some(s) = extract_export_entry(dot)
        {
            return Some(s.to_string());
        }
    }

    // module field (ESM entry point)
    if let Some(s) = json.get("module").and_then(|v| v.as_str()) {
        return Some(s.to_string());
    }

    // main field
    if let Some(s) = json.get("main").and_then(|v| v.as_str()) {
        return Some(s.to_string());
    }

    None
}

fn extract_export_entry(value: &Value) -> Option<&str> {
    if let Some(s) = value.as_str() {
        return Some(s);
    }
    if let Some(obj) = value.as_object() {
        for key in &["import", "require", "default"] {
            if let Some(entry) = obj.get(*key) {
                if let Some(s) = entry.as_str() {
                    return Some(s);
                }
                if let Some(s) = extract_export_entry(entry) {
                    return Some(s);
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ManifestParser;

    #[test]
    fn test_parse_package_json() {
        let content = r#"{
  "name": "my-app",
  "version": "1.2.3",
  "dependencies": {
    "express": "^4.18.0",
    "lodash": "^4.17.21"
  },
  "devDependencies": {
    "jest": "^29.0.0"
  }
}"#;
        let m = NpmParser.parse(content).unwrap();
        assert_eq!(m.ecosystem, "npm");
        assert_eq!(m.name.as_deref(), Some("my-app"));
        assert_eq!(m.version.as_deref(), Some("1.2.3"));
        assert_eq!(m.dependencies.len(), 3);

        let normal: Vec<_> = m
            .dependencies
            .iter()
            .filter(|d| d.kind == DepKind::Normal)
            .collect();
        assert_eq!(normal.len(), 2);

        let dev: Vec<_> = m
            .dependencies
            .iter()
            .filter(|d| d.kind == DepKind::Dev)
            .collect();
        assert_eq!(dev.len(), 1);
        assert_eq!(dev[0].name, "jest");
    }

    #[test]
    fn test_npm_entry_point_main() {
        let content = r#"{"main": "dist/index.js"}"#;
        assert_eq!(npm_entry_point(content).as_deref(), Some("dist/index.js"));
    }

    #[test]
    fn test_npm_entry_point_module() {
        let content = r#"{"module": "esm/index.js", "main": "cjs/index.js"}"#;
        // module takes precedence over main
        assert_eq!(npm_entry_point(content).as_deref(), Some("esm/index.js"));
    }

    #[test]
    fn test_npm_entry_point_exports_string() {
        let content = r#"{"exports": "./dist/index.js"}"#;
        assert_eq!(npm_entry_point(content).as_deref(), Some("./dist/index.js"));
    }

    #[test]
    fn test_npm_entry_point_exports_dot() {
        let content =
            r#"{"exports": {".": {"import": "./esm/index.js", "require": "./cjs/index.js"}}}"#;
        assert_eq!(npm_entry_point(content).as_deref(), Some("./esm/index.js"));
    }

    #[test]
    fn test_npm_entry_point_missing() {
        let content = r#"{"name": "no-entry"}"#;
        assert_eq!(npm_entry_point(content), None);
    }
}
