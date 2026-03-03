//! Parser for `requirements.txt` files (pip).

use crate::{DeclaredDep, DepKind, ManifestError, ManifestParser, ParsedManifest};

/// Parser for `requirements.txt` files.
pub struct PipParser;

impl ManifestParser for PipParser {
    fn filename(&self) -> &'static str {
        "requirements.txt"
    }

    fn parse(&self, content: &str) -> Result<ParsedManifest, ManifestError> {
        let mut deps = Vec::new();

        for line in content.lines() {
            let line = line.trim();
            // Skip comments, empty lines, and pip options (-r, -c, --index-url, etc.)
            if line.is_empty() || line.starts_with('#') || line.starts_with('-') {
                continue;
            }
            // Strip inline comment
            let line = match line.find(" #") {
                Some(idx) => line[..idx].trim(),
                None => line,
            };
            if let Some(dep) = parse_pip_requirement(line) {
                deps.push(dep);
            }
        }

        Ok(ParsedManifest {
            ecosystem: "pip",
            name: None,
            version: None,
            dependencies: deps,
        })
    }
}

fn parse_pip_requirement(line: &str) -> Option<DeclaredDep> {
    let line = line.trim();
    if line.is_empty() {
        return None;
    }
    // Split on version operators in order of specificity
    const OPERATORS: &[&str] = &["===", "~=", "==", "!=", ">=", "<=", ">", "<"];
    for op in OPERATORS {
        if let Some(idx) = line.find(op) {
            let name = line[..idx].trim().to_string();
            if name.is_empty() {
                continue;
            }
            let version_req = Some(line[idx..].trim().to_string());
            return Some(DeclaredDep {
                name,
                version_req,
                kind: DepKind::Normal,
            });
        }
    }
    // No version specifier — bare package name
    if !line.is_empty() {
        return Some(DeclaredDep {
            name: line.to_string(),
            version_req: None,
            kind: DepKind::Normal,
        });
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ManifestParser;

    #[test]
    fn test_parse_requirements_txt() {
        let content = r#"# Production dependencies
requests==2.28.0
flask>=2.0
numpy  # scientific computing
# dev
pytest
"#;
        let m = PipParser.parse(content).unwrap();
        assert_eq!(m.ecosystem, "pip");
        assert!(m.name.is_none());
        assert_eq!(m.dependencies.len(), 4);

        let requests = m
            .dependencies
            .iter()
            .find(|d| d.name == "requests")
            .unwrap();
        assert_eq!(requests.version_req.as_deref(), Some("==2.28.0"));

        let flask = m.dependencies.iter().find(|d| d.name == "flask").unwrap();
        assert_eq!(flask.version_req.as_deref(), Some(">=2.0"));

        let numpy = m.dependencies.iter().find(|d| d.name == "numpy").unwrap();
        assert!(numpy.version_req.is_none());

        let pytest = m.dependencies.iter().find(|d| d.name == "pytest").unwrap();
        assert!(pytest.version_req.is_none());
    }

    #[test]
    fn test_skip_pip_options() {
        let content = "-r base.txt\n--index-url https://pypi.org\nrequests\n";
        let m = PipParser.parse(content).unwrap();
        assert_eq!(m.dependencies.len(), 1);
        assert_eq!(m.dependencies[0].name, "requests");
    }
}
