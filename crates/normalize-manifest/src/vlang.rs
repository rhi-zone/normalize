//! Parser for `v.mod` files (V language/vpm).
//!
//! V module files use a simple key-value syntax. We heuristically extract
//! `name`, `version`, and `dependencies` fields using line-based matching.

use crate::{DeclaredDep, DepKind, ManifestError, ManifestParser, ParsedManifest};

/// Parser for `v.mod` files.
pub struct VModParser;

impl ManifestParser for VModParser {
    fn filename(&self) -> &'static str {
        "v.mod"
    }

    fn parse(&self, content: &str) -> Result<ParsedManifest, ManifestError> {
        let mut name: Option<String> = None;
        let mut version: Option<String> = None;
        let mut deps: Vec<DeclaredDep> = Vec::new();

        let mut in_deps = false;

        for line in content.lines() {
            let trimmed = line.trim();

            if trimmed.is_empty() || trimmed.starts_with("//") {
                continue;
            }

            // Enter/exit the `dependencies: [...]` block.
            if trimmed.starts_with("dependencies:") {
                in_deps = true;
                // Check if the whole list is on one line: dependencies: ['a', 'b']
                if let Some(rest) = trimmed.strip_prefix("dependencies:") {
                    let rest = rest.trim();
                    if rest.starts_with('[') {
                        extract_vmod_list(rest, &mut deps);
                        if rest.contains(']') {
                            in_deps = false;
                        }
                    }
                }
                continue;
            }

            if in_deps {
                extract_vmod_list(trimmed, &mut deps);
                if trimmed.contains(']') {
                    in_deps = false;
                }
                continue;
            }

            // name: 'mymodule'
            if let Some(rest) = trimmed.strip_prefix("name:") {
                if name.is_none() {
                    let v = extract_single_quoted(rest.trim())
                        .or_else(|| extract_double_quoted(rest.trim()))
                        .unwrap_or_else(|| rest.trim().to_string());
                    if !v.is_empty() {
                        name = Some(v);
                    }
                }
                continue;
            }

            // version: '0.1.0'
            if let Some(rest) = trimmed.strip_prefix("version:") {
                if version.is_none() {
                    let v = extract_single_quoted(rest.trim())
                        .or_else(|| extract_double_quoted(rest.trim()))
                        .unwrap_or_else(|| rest.trim().to_string());
                    if !v.is_empty() {
                        version = Some(v);
                    }
                }
                continue;
            }
        }

        Ok(ParsedManifest {
            ecosystem: "vpm",
            name,
            version,
            dependencies: deps,
        })
    }
}

/// Extract single-quoted string: `'value'` → `value`.
fn extract_single_quoted(s: &str) -> Option<String> {
    let s = s.trim();
    let inner = s.strip_prefix('\'')?;
    let end = inner.find('\'')?;
    Some(inner[..end].to_string())
}

/// Extract double-quoted string: `"value"` → `value`.
fn extract_double_quoted(s: &str) -> Option<String> {
    let s = s.trim();
    let inner = s.strip_prefix('"')?;
    let end = inner.find('"')?;
    Some(inner[..end].to_string())
}

/// Extract all single-quoted dep entries from a fragment like `['a', 'b.c']`.
fn extract_vmod_list(fragment: &str, deps: &mut Vec<DeclaredDep>) {
    let mut s = fragment;
    while let Some(start) = s.find('\'') {
        s = &s[start + 1..];
        if let Some(end) = s.find('\'') {
            let dep_name = s[..end].trim().to_string();
            if !dep_name.is_empty() {
                deps.push(DeclaredDep {
                    name: dep_name,
                    version_req: None,
                    kind: DepKind::Normal,
                });
            }
            s = &s[end + 1..];
        } else {
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ManifestParser;

    const SAMPLE: &str = r#"Module {
  name: 'mymodule'
  description: 'My V module'
  version: '0.1.0'
  license: 'MIT'
  dependencies: ['vweb', 'json', 'db.sqlite']
}
"#;

    #[test]
    fn test_parse_vmod() {
        let m = VModParser.parse(SAMPLE).unwrap();
        assert_eq!(m.ecosystem, "vpm");
        assert_eq!(m.name.as_deref(), Some("mymodule"));
        assert_eq!(m.version.as_deref(), Some("0.1.0"));

        let names: Vec<&str> = m.dependencies.iter().map(|d| d.name.as_str()).collect();
        assert!(names.contains(&"vweb"), "{names:?}");
        assert!(names.contains(&"json"), "{names:?}");
        assert!(names.contains(&"db.sqlite"), "{names:?}");
        assert_eq!(m.dependencies.len(), 3);
    }

    #[test]
    fn test_multiline_deps() {
        let content = r#"Module {
  name: 'multi'
  version: '0.2.0'
  dependencies: [
    'a',
    'b',
    'c'
  ]
}
"#;
        let m = VModParser.parse(content).unwrap();
        let names: Vec<&str> = m.dependencies.iter().map(|d| d.name.as_str()).collect();
        assert_eq!(names, vec!["a", "b", "c"], "{names:?}");
    }

    #[test]
    fn test_no_deps() {
        let content = r#"Module {
  name: 'simple'
  version: '1.0.0'
}
"#;
        let m = VModParser.parse(content).unwrap();
        assert_eq!(m.name.as_deref(), Some("simple"));
        assert!(m.dependencies.is_empty());
    }
}
