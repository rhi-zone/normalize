//! Parser for `stack.yaml` files (Haskell/Stack).
//!
//! Extracts `extra-deps:` entries. Stack uses Stackage snapshots for most deps;
//! `extra-deps` lists packages not on the snapshot (usually pinned versions or git).
//!
//! Uses indent-aware line parsing rather than a full YAML library.

use crate::{DeclaredDep, DepKind, ManifestError, ManifestParser, ParsedManifest};

/// Parser for `stack.yaml` files.
pub struct StackParser;

impl ManifestParser for StackParser {
    fn filename(&self) -> &'static str {
        "stack.yaml"
    }

    fn parse(&self, content: &str) -> Result<ParsedManifest, ManifestError> {
        let mut deps = Vec::new();
        let mut in_extra_deps = false;
        let mut list_indent: usize = 0;

        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }

            let indent = line.len() - line.trim_start().len();

            // Top-level key detection
            if indent == 0 {
                in_extra_deps = trimmed.starts_with("extra-deps:");
                list_indent = 0;
                // Handle inline list: `extra-deps: []`
                if in_extra_deps && trimmed.contains('[') {
                    // Empty or inline — not common, skip
                    in_extra_deps = false;
                }
                continue;
            }

            if !in_extra_deps {
                continue;
            }

            // List items start with `- `
            if trimmed.starts_with("- ") || trimmed.starts_with('-') {
                if list_indent == 0 {
                    list_indent = indent;
                }

                let item = trimmed.trim_start_matches('-').trim();

                if let Some(dep) = parse_stack_dep(item) {
                    deps.push(dep);
                }
            }
        }

        Ok(ParsedManifest {
            ecosystem: "stackage",
            name: None,
            version: None,
            dependencies: deps,
        })
    }
}

fn parse_stack_dep(item: &str) -> Option<DeclaredDep> {
    let item = item.trim().trim_matches('"').trim_matches('\'');
    if item.is_empty() {
        return None;
    }

    // Git dep: `git: ...` (multi-line object, heuristic: skip, we can't fully parse)
    if item == "git:" || item.starts_with("git:") {
        return None;
    }

    // Hackage form: `pkg-name-1.2.3`  or  `pkg-name-1.2.3@sha256:...`
    // The package name uses hyphens; version is the last hyphenated segment starting with digit
    let base = item.split('@').next().unwrap_or(item);

    // Find where the version starts (last hyphen before a digit)
    let name;
    let version_req;

    if let Some(ver_start) = find_version_start(base) {
        name = base[..ver_start - 1].to_string(); // strip trailing hyphen
        version_req = Some(base[ver_start..].to_string());
    } else {
        name = base.to_string();
        version_req = None;
    }

    if name.is_empty() {
        return None;
    }

    Some(DeclaredDep {
        name,
        version_req,
        kind: DepKind::Normal,
    })
}

/// Find the index where the version part starts in a `pkg-name-1.2.3` string.
/// Returns the index of the first digit of the version (after the separating hyphen).
fn find_version_start(s: &str) -> Option<usize> {
    let bytes = s.as_bytes();
    // Walk backwards from the end to find last hyphen before a digit sequence
    (1..bytes.len())
        .rev()
        .find(|&i| bytes[i - 1] == b'-' && bytes[i].is_ascii_digit())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ManifestParser;

    #[test]
    fn test_parse_stack_yaml() {
        let content = r#"resolver: lts-21.0

packages:
  - .

extra-deps:
  - acme-pkg-1.2.3
  - aeson-2.1.2.1
  - text-2.0.2@sha256:abc123
"#;
        let m = StackParser.parse(content).unwrap();
        assert_eq!(m.ecosystem, "stackage");
        assert_eq!(m.dependencies.len(), 3);

        let acme = m
            .dependencies
            .iter()
            .find(|d| d.name == "acme-pkg")
            .unwrap();
        assert_eq!(acme.version_req.as_deref(), Some("1.2.3"));

        let text = m.dependencies.iter().find(|d| d.name == "text").unwrap();
        assert_eq!(text.version_req.as_deref(), Some("2.0.2"));
    }
}
