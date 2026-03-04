//! Parser for `Project.toml` files (Julia package manager).

use crate::{DeclaredDep, DepKind, ManifestError, ManifestParser, ParsedManifest};
use toml::Value;

/// Parser for Julia `Project.toml` files.
///
/// - `[deps]` → `Normal`. UUID values are package identifiers; version comes from
///   `[compat]` if present, otherwise `None`.
/// - `[weakdeps]` → `Optional`. Same version lookup via `[compat]`.
/// - `[compat]` entries whose key is `"julia"` are skipped (not a dep).
pub struct JuliaParser;

impl ManifestParser for JuliaParser {
    fn filename(&self) -> &'static str {
        "Project.toml"
    }

    fn parse(&self, content: &str) -> Result<ParsedManifest, ManifestError> {
        let toml: Value = content
            .parse::<Value>()
            .map_err(|e| ManifestError(e.to_string()))?;

        let name = toml
            .get("name")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let version = toml
            .get("version")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        // Build compat map: package name -> version constraint
        let compat: std::collections::HashMap<String, String> = toml
            .get("compat")
            .and_then(|v| v.as_table())
            .map(|t| {
                t.iter()
                    .filter(|(k, _)| k.as_str() != "julia")
                    .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                    .collect()
            })
            .unwrap_or_default();

        let mut deps = Vec::new();

        // [deps] - Normal dependencies (values are UUIDs, not versions)
        if let Some(table) = toml.get("deps").and_then(|v| v.as_table()) {
            for (pkg_name, _uuid) in table {
                let version_req = compat.get(pkg_name).cloned();
                deps.push(DeclaredDep {
                    name: pkg_name.clone(),
                    version_req,
                    kind: DepKind::Normal,
                });
            }
        }

        // [weakdeps] - Optional dependencies
        if let Some(table) = toml.get("weakdeps").and_then(|v| v.as_table()) {
            for (pkg_name, _uuid) in table {
                let version_req = compat.get(pkg_name).cloned();
                deps.push(DeclaredDep {
                    name: pkg_name.clone(),
                    version_req,
                    kind: DepKind::Optional,
                });
            }
        }

        Ok(ParsedManifest {
            ecosystem: "julia",
            name,
            version,
            dependencies: deps,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ManifestParser;

    #[test]
    fn test_julia_project_toml() {
        let content = r#"
name = "MyPackage"
uuid = "a93c6f00-e57d-5684-b466-afe8fa294f38"
version = "0.1.0"

[deps]
DataFrames = "a93c6f00-e57d-5684-b466-afe8fa294f38"
HTTP = "cd3eb016-35fb-5094-929b-558a96fad6f3"

[weakdeps]
CUDA = "052768ef-5323-5732-b1bb-66c8b64840ba"

[compat]
julia = "1.6"
DataFrames = "1"
HTTP = "0.9, 1"
"#;
        let m = JuliaParser.parse(content).unwrap();
        assert_eq!(m.ecosystem, "julia");
        assert_eq!(m.name.as_deref(), Some("MyPackage"));
        assert_eq!(m.version.as_deref(), Some("0.1.0"));
        assert_eq!(m.dependencies.len(), 3);

        let df = m
            .dependencies
            .iter()
            .find(|d| d.name == "DataFrames")
            .unwrap();
        assert_eq!(df.kind, DepKind::Normal);
        assert_eq!(df.version_req.as_deref(), Some("1"));

        let http = m.dependencies.iter().find(|d| d.name == "HTTP").unwrap();
        assert_eq!(http.kind, DepKind::Normal);
        assert_eq!(http.version_req.as_deref(), Some("0.9, 1"));

        let cuda = m.dependencies.iter().find(|d| d.name == "CUDA").unwrap();
        assert_eq!(cuda.kind, DepKind::Optional);
        assert!(cuda.version_req.is_none());
    }

    #[test]
    fn test_julia_no_compat() {
        let content = r#"
name = "Simple"
version = "0.1.0"

[deps]
JSON = "682c06a0-de6a-54ab-a142-c8b1cf79cde6"
"#;
        let m = JuliaParser.parse(content).unwrap();
        assert_eq!(m.dependencies.len(), 1);
        assert_eq!(m.dependencies[0].name, "JSON");
        assert!(m.dependencies[0].version_req.is_none());
        assert_eq!(m.dependencies[0].kind, DepKind::Normal);
    }

    #[test]
    fn test_julia_compat_julia_skipped() {
        // Ensure "julia" entry in [compat] is not emitted as a dep
        let content = r#"
name = "OnlyJuliaCompat"
version = "0.1.0"

[deps]
Plots = "91a5bcdd-55d7-5caf-9e0b-520d859cae80"

[compat]
julia = "1.9"
Plots = "1"
"#;
        let m = JuliaParser.parse(content).unwrap();
        // Only "Plots" should be in deps, not "julia"
        assert_eq!(m.dependencies.len(), 1);
        assert_eq!(m.dependencies[0].name, "Plots");
    }
}
