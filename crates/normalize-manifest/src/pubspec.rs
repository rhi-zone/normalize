//! Parser for `pubspec.yaml` files (Dart/Flutter).
//!
//! Uses indent-aware line parsing rather than a full YAML library.
//! Extracts `dependencies:` and `dev_dependencies:` sections.

use crate::{DeclaredDep, DepKind, ManifestError, ManifestParser, ParsedManifest};

/// Parser for `pubspec.yaml` files.
pub struct PubspecParser;

impl ManifestParser for PubspecParser {
    fn filename(&self) -> &'static str {
        "pubspec.yaml"
    }

    fn parse(&self, content: &str) -> Result<ParsedManifest, ManifestError> {
        let mut name = None;
        let mut version = None;
        let mut deps = Vec::new();

        #[derive(PartialEq, Clone, Copy)]
        enum Section {
            None,
            Deps,
            DevDeps,
        }

        let mut section = Section::None;
        let mut section_indent = 0usize;
        let mut dep_name: Option<(String, DepKind)> = None;

        for line in content.lines() {
            if line.trim().is_empty() || line.trim().starts_with('#') {
                continue;
            }

            let indent = leading_spaces(line);
            let trimmed = line.trim();

            // Top-level keys (indent == 0)
            if indent == 0 {
                // Flush any pending dep with no version
                if let Some((dname, dkind)) = dep_name.take() {
                    deps.push(DeclaredDep {
                        name: dname,
                        version_req: None,
                        kind: dkind,
                    });
                }

                if let Some(rest) = trimmed.strip_prefix("name:") {
                    name = rest
                        .trim()
                        .trim_matches('"')
                        .trim_matches('\'')
                        .to_string()
                        .into();
                    section = Section::None;
                } else if let Some(rest) = trimmed.strip_prefix("version:") {
                    version = rest
                        .trim()
                        .trim_matches('"')
                        .trim_matches('\'')
                        .to_string()
                        .into();
                    section = Section::None;
                } else if trimmed == "dependencies:" {
                    section = Section::Deps;
                    section_indent = 0;
                } else if trimmed == "dev_dependencies:" {
                    section = Section::DevDeps;
                    section_indent = 0;
                } else {
                    section = Section::None;
                }
                continue;
            }

            if section == Section::None {
                continue;
            }

            let kind = if section == Section::DevDeps {
                DepKind::Dev
            } else {
                DepKind::Normal
            };

            // First-level entries under the section
            if section_indent == 0 || indent == section_indent {
                // Flush previous dep
                if let Some((dname, dkind)) = dep_name.take() {
                    deps.push(DeclaredDep {
                        name: dname,
                        version_req: None,
                        kind: dkind,
                    });
                }

                section_indent = indent;

                // `  flutter:` or `  http: ^0.13.0`
                if let Some(colon_idx) = trimmed.find(':') {
                    let pkg_name = trimmed[..colon_idx].trim().to_string();
                    let after_colon = trimmed[colon_idx + 1..].trim();

                    // Skip `sdk: flutter` — it's a platform dep, not a package
                    if after_colon == "sdk: flutter"
                        || after_colon == "flutter"
                        || after_colon.starts_with("sdk:")
                    {
                        continue;
                    }

                    if after_colon.is_empty() {
                        // Version (or sub-keys) on next line(s)
                        dep_name = Some((pkg_name, kind));
                    } else {
                        // Inline version: `http: ^0.13.0`
                        deps.push(DeclaredDep {
                            name: pkg_name,
                            version_req: Some(after_colon.to_string()),
                            kind,
                        });
                    }
                }
            } else if let Some((ref dname, dkind)) = dep_name {
                // Sub-key of a dep: `version: ^1.0.0`, `path: ../pkg`, `sdk: flutter`, etc.
                if trimmed.starts_with("sdk:") {
                    // Platform/SDK dep — discard, not a real package
                    dep_name = None;
                } else if let Some(ver_rest) = trimmed.strip_prefix("version:") {
                    let ver = ver_rest.trim().to_string();
                    deps.push(DeclaredDep {
                        name: dname.clone(),
                        version_req: if ver.is_empty() { None } else { Some(ver) },
                        kind: dkind,
                    });
                    dep_name = None;
                }
                // git/path deps: dep_name remains; flushed at next sibling entry with no version_req
            }
        }

        // Flush last pending dep
        if let Some((dname, dkind)) = dep_name.take() {
            deps.push(DeclaredDep {
                name: dname,
                version_req: None,
                kind: dkind,
            });
        }

        Ok(ParsedManifest {
            ecosystem: "pub",
            name,
            version,
            dependencies: deps,
        })
    }
}

fn leading_spaces(line: &str) -> usize {
    line.len() - line.trim_start().len()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ManifestParser;

    #[test]
    fn test_parse_pubspec_yaml() {
        let content = r#"name: my_flutter_app
version: 1.0.0+1
description: A new Flutter project.

dependencies:
  flutter:
    sdk: flutter
  http: ^0.13.0
  provider:
    version: ^6.0.0
  path_provider: ^2.0.0

dev_dependencies:
  flutter_test:
    sdk: flutter
  mockito: ^5.0.0
"#;
        let m = PubspecParser.parse(content).unwrap();
        assert_eq!(m.ecosystem, "pub");
        assert_eq!(m.name.as_deref(), Some("my_flutter_app"));
        assert_eq!(m.version.as_deref(), Some("1.0.0+1"));

        let normal: Vec<_> = m
            .dependencies
            .iter()
            .filter(|d| d.kind == DepKind::Normal)
            .collect();
        // flutter (sdk) is filtered, http + provider + path_provider remain
        assert_eq!(normal.len(), 3);
        assert!(normal.iter().any(|d| d.name == "http"));

        let http = m.dependencies.iter().find(|d| d.name == "http").unwrap();
        assert_eq!(http.version_req.as_deref(), Some("^0.13.0"));

        let dev: Vec<_> = m
            .dependencies
            .iter()
            .filter(|d| d.kind == DepKind::Dev)
            .collect();
        // flutter_test (sdk) is filtered, mockito remains
        assert_eq!(dev.len(), 1);
        assert_eq!(dev[0].name, "mockito");
    }
}
