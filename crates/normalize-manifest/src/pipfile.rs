//! Parser for `Pipfile` files (Pipenv).

use crate::{DeclaredDep, DepKind, ManifestError, ManifestParser, ParsedManifest};
use toml::Value;

/// Parser for `Pipfile` files (Pipenv).
pub struct PipfileParser;

impl ManifestParser for PipfileParser {
    fn filename(&self) -> &'static str {
        "Pipfile"
    }

    fn parse(&self, content: &str) -> Result<ParsedManifest, ManifestError> {
        let toml: Value = content
            .parse::<Value>()
            .map_err(|e| ManifestError(e.to_string()))?;

        let mut deps = Vec::new();

        parse_section(&toml, "packages", DepKind::Normal, &mut deps);
        parse_section(&toml, "dev-packages", DepKind::Dev, &mut deps);

        Ok(ParsedManifest {
            ecosystem: "pip",
            name: None,
            version: None,
            dependencies: deps,
        })
    }
}

/// Parse a `[packages]` or `[dev-packages]` section into `deps`.
fn parse_section(toml: &Value, section: &str, kind: DepKind, deps: &mut Vec<DeclaredDep>) {
    let Some(table) = toml.get(section).and_then(|v| v.as_table()) else {
        return;
    };

    for (name, val) in table {
        let version_req = dep_version_req(val);
        deps.push(DeclaredDep {
            name: name.clone(),
            version_req,
            kind,
        });
    }
}

/// Extract the version requirement from a Pipfile dependency value.
///
/// - `"*"` → `None`
/// - `">=2.0"` (any other string) → `Some(">=2.0")`
/// - `{version = ">=2.0", ...}` → `Some(">=2.0")`, or `None` if `"*"`
/// - `{git = "...", ...}` (VCS/path table without `version`) → `None`
fn dep_version_req(val: &Value) -> Option<String> {
    match val {
        Value::String(s) => version_string(s),
        Value::Table(t) => t
            .get("version")
            .and_then(|v| v.as_str())
            .and_then(version_string),
        _ => None,
    }
}

/// Convert a version string to `Option<String>`, treating `"*"` as `None`.
fn version_string(s: &str) -> Option<String> {
    if s == "*" { None } else { Some(s.to_string()) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ManifestParser;

    const SAMPLE: &str = r#"
[[source]]
url = "https://pypi.org/simple"
verify_ssl = true
name = "pypi"

[packages]
requests = "*"
flask = ">=2.0"
click = {version = ">=8.0", extras = ["dev"]}
django = {version = "*", markers = "python_version >= '3.8'"}
sqlalchemy = {git = "https://github.com/sqlalchemy/sqlalchemy.git", ref = "main"}

[dev-packages]
pytest = ">=7.0"
black = "*"
mypy = {version = ">=1.0"}

[requires]
python_version = "3.11"
"#;

    #[test]
    fn test_ecosystem_and_no_name() {
        let m = PipfileParser.parse(SAMPLE).unwrap();
        assert_eq!(m.ecosystem, "pip");
        assert!(m.name.is_none());
        assert!(m.version.is_none());
    }

    #[test]
    fn test_normal_deps() {
        let m = PipfileParser.parse(SAMPLE).unwrap();
        let normal: Vec<_> = m
            .dependencies
            .iter()
            .filter(|d| d.kind == DepKind::Normal)
            .collect();
        assert_eq!(normal.len(), 5);

        // "*" → no version
        let requests = normal.iter().find(|d| d.name == "requests").unwrap();
        assert!(requests.version_req.is_none());

        // plain version string
        let flask = normal.iter().find(|d| d.name == "flask").unwrap();
        assert_eq!(flask.version_req.as_deref(), Some(">=2.0"));

        // inline table with version
        let click = normal.iter().find(|d| d.name == "click").unwrap();
        assert_eq!(click.version_req.as_deref(), Some(">=8.0"));

        // inline table with version = "*" → no version
        let django = normal.iter().find(|d| d.name == "django").unwrap();
        assert!(django.version_req.is_none());

        // VCS table without version key → no version
        let sqlalchemy = normal.iter().find(|d| d.name == "sqlalchemy").unwrap();
        assert!(sqlalchemy.version_req.is_none());
    }

    #[test]
    fn test_dev_deps() {
        let m = PipfileParser.parse(SAMPLE).unwrap();
        let dev: Vec<_> = m
            .dependencies
            .iter()
            .filter(|d| d.kind == DepKind::Dev)
            .collect();
        assert_eq!(dev.len(), 3);

        let pytest = dev.iter().find(|d| d.name == "pytest").unwrap();
        assert_eq!(pytest.version_req.as_deref(), Some(">=7.0"));

        let black = dev.iter().find(|d| d.name == "black").unwrap();
        assert!(black.version_req.is_none());

        let mypy = dev.iter().find(|d| d.name == "mypy").unwrap();
        assert_eq!(mypy.version_req.as_deref(), Some(">=1.0"));
    }

    #[test]
    fn test_empty_pipfile() {
        let m = PipfileParser.parse("").unwrap();
        assert!(m.dependencies.is_empty());
    }

    #[test]
    fn test_no_dev_section() {
        let content = r#"
[packages]
requests = ">=2.28"
"#;
        let m = PipfileParser.parse(content).unwrap();
        assert_eq!(m.dependencies.len(), 1);
        assert_eq!(m.dependencies[0].kind, DepKind::Normal);
        assert_eq!(m.dependencies[0].version_req.as_deref(), Some(">=2.28"));
    }
}
