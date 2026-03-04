//! Parser for `spago.yaml` files (PureScript/Spago).
//!
//! We use heuristic line-based parsing — no full YAML parser. The file has a
//! well-known structure: `package:` section with `name:`, `version:`, and
//! `dependencies:` keys.

use crate::{DeclaredDep, DepKind, ManifestError, ManifestParser, ParsedManifest};

/// Parser for `spago.yaml` files.
pub struct SpagoParser;

impl ManifestParser for SpagoParser {
    fn filename(&self) -> &'static str {
        "spago.yaml"
    }

    fn parse(&self, content: &str) -> Result<ParsedManifest, ManifestError> {
        let mut name: Option<String> = None;
        let mut version: Option<String> = None;
        let mut deps: Vec<DeclaredDep> = Vec::new();

        #[derive(PartialEq, Clone, Copy)]
        enum Section {
            None,
            Package,
            Dependencies,
        }

        let mut section = Section::None;

        for line in content.lines() {
            // Detect section headers (zero-indented keys).
            let trimmed = line.trim();

            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }

            // Zero-indented keys switch sections.
            if !line.starts_with(' ') && !line.starts_with('\t') {
                if trimmed.starts_with("package:") {
                    section = Section::Package;
                } else {
                    section = Section::None;
                }
                continue;
            }

            // Indented keys within the `package:` section.
            if section == Section::Package {
                // Two-space indented keys: name, version, dependencies.
                let stripped = trimmed;
                if let Some(rest) = stripped.strip_prefix("name:") {
                    let v = rest.trim().trim_matches('"').trim_matches('\'').to_string();
                    if !v.is_empty() && name.is_none() {
                        name = Some(v);
                    }
                    continue;
                }
                if let Some(rest) = stripped.strip_prefix("version:") {
                    let v = rest.trim().trim_matches('"').trim_matches('\'').to_string();
                    if !v.is_empty() && version.is_none() {
                        version = Some(v);
                    }
                    continue;
                }
                if stripped.starts_with("dependencies:") {
                    section = Section::Dependencies;
                    // The value might be on the same line (unlikely for lists).
                    continue;
                }
                // Other indented keys in package section — stay in Package.
                continue;
            }

            if section == Section::Dependencies {
                // Dependency list items start with `- `.
                if let Some(rest) = trimmed.strip_prefix("- ") {
                    if let Some(dep) = parse_spago_dep(rest) {
                        deps.push(dep);
                    }
                    continue;
                }
                // A non-list-item at the same or lower indent means we've
                // left the dependencies list. If this is another key in the
                // package section, go back to Package; otherwise, None.
                if !trimmed.starts_with('-') {
                    // Check if it's another package-level key.
                    section = Section::Package;
                }
                continue;
            }
        }

        Ok(ParsedManifest {
            ecosystem: "pursuit",
            name,
            version,
            dependencies: deps,
        })
    }
}

/// Parse a single dependency entry: bare name or `name: "version_range"`.
fn parse_spago_dep(rest: &str) -> Option<DeclaredDep> {
    let rest = rest.trim();
    if rest.is_empty() {
        return None;
    }

    // Inline mapping: `pkgname: ">=7.0.0 <8.0.0"` (YAML inline notation)
    if let Some(colon_pos) = rest.find(':') {
        let pkg_name = rest[..colon_pos].trim().to_string();
        let ver_str = rest[colon_pos + 1..].trim();
        let version_req = if ver_str.is_empty() {
            None
        } else {
            Some(ver_str.trim_matches('"').trim_matches('\'').to_string())
        };
        if !pkg_name.is_empty() {
            return Some(DeclaredDep {
                name: pkg_name,
                version_req,
                kind: DepKind::Normal,
            });
        }
    }

    // Bare package name (strip trailing quote/whitespace noise).
    let pkg_name = rest.trim_matches('"').trim_matches('\'').trim().to_string();
    if pkg_name.is_empty() {
        return None;
    }
    Some(DeclaredDep {
        name: pkg_name,
        version_req: None,
        kind: DepKind::Normal,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ManifestParser;

    const SAMPLE: &str = r#"package:
  name: my-project
  version: 0.1.0
  dependencies:
    - prelude
    - effect
    - console
    - aff: ">=7.0.0 <8.0.0"

workspace:
  extra_packages: {}
"#;

    #[test]
    fn test_parse_spago() {
        let m = SpagoParser.parse(SAMPLE).unwrap();
        assert_eq!(m.ecosystem, "pursuit");
        assert_eq!(m.name.as_deref(), Some("my-project"));
        assert_eq!(m.version.as_deref(), Some("0.1.0"));

        let names: Vec<&str> = m.dependencies.iter().map(|d| d.name.as_str()).collect();
        assert!(names.contains(&"prelude"), "{names:?}");
        assert!(names.contains(&"effect"), "{names:?}");
        assert!(names.contains(&"console"), "{names:?}");
        assert!(names.contains(&"aff"), "{names:?}");

        let aff = m.dependencies.iter().find(|d| d.name == "aff").unwrap();
        assert_eq!(aff.version_req.as_deref(), Some(">=7.0.0 <8.0.0"));
    }

    #[test]
    fn test_workspace_excluded() {
        let content = r#"package:
  name: lib
  version: 1.0.0
  dependencies:
    - prelude

workspace:
  extra_packages: {}
"#;
        let m = SpagoParser.parse(content).unwrap();
        assert_eq!(m.name.as_deref(), Some("lib"));
        assert_eq!(m.dependencies.len(), 1);
    }
}
