//! Parser for `cabal.project` files (Haskell/Cabal).
//!
//! Cabal project files configure multi-package Haskell projects. We
//! heuristically extract `source-repository-package` stanzas, treating them
//! as external (git-pinned) dependencies. The `location:` field provides the
//! URL and the `tag:` field acts as the version requirement.

use crate::{DeclaredDep, DepKind, ManifestError, ManifestParser, ParsedManifest};

/// Parser for `cabal.project` files.
pub struct CabalProjectParser;

impl ManifestParser for CabalProjectParser {
    fn filename(&self) -> &'static str {
        "cabal.project"
    }

    fn parse(&self, content: &str) -> Result<ParsedManifest, ManifestError> {
        let mut deps: Vec<DeclaredDep> = Vec::new();

        // We walk stanzas. A stanza starts at a zero-indented keyword line.
        // Continuations are indented.

        #[derive(PartialEq)]
        enum Stanza {
            Other,
            SourceRepoPackage,
        }

        let mut stanza = Stanza::Other;
        let mut current_location: Option<String> = None;
        let mut current_tag: Option<String> = None;

        let flush =
            |loc: &mut Option<String>, tag: &mut Option<String>, deps: &mut Vec<DeclaredDep>| {
                if let Some(url) = loc.take() {
                    let dep_name = derive_name_from_url(&url);
                    deps.push(DeclaredDep {
                        name: dep_name,
                        version_req: tag.take(),
                        kind: DepKind::Normal,
                    });
                } else {
                    tag.take();
                }
            };

        for line in content.lines() {
            let trimmed = line.trim();

            // Skip blanks and comments.
            if trimmed.is_empty() || trimmed.starts_with("--") {
                continue;
            }

            // Zero-indented line = new stanza header.
            if !line.starts_with(' ') && !line.starts_with('\t') {
                // Flush previous source-repo-package stanza.
                if stanza == Stanza::SourceRepoPackage {
                    flush(&mut current_location, &mut current_tag, &mut deps);
                }

                let lower = trimmed.to_lowercase();
                if lower.starts_with("source-repository-package") {
                    stanza = Stanza::SourceRepoPackage;
                } else {
                    stanza = Stanza::Other;
                }
                continue;
            }

            // Indented fields within a stanza.
            if stanza == Stanza::SourceRepoPackage {
                if let Some(rest) = trimmed.strip_prefix("location:") {
                    current_location = Some(rest.trim().to_string());
                } else if let Some(rest) = trimmed.strip_prefix("tag:") {
                    current_tag = Some(rest.trim().to_string());
                }
            }
        }

        // Flush the last stanza.
        if stanza == Stanza::SourceRepoPackage {
            flush(&mut current_location, &mut current_tag, &mut deps);
        }

        Ok(ParsedManifest {
            ecosystem: "cabal",
            // cabal.project doesn't declare a single package name/version.
            name: None,
            version: None,
            dependencies: deps,
        })
    }
}

/// Derive a dependency name from a git URL.
///
/// Strips the `.git` suffix and takes the last path component.
/// E.g. `https://github.com/someone/something.git` → `something`.
fn derive_name_from_url(url: &str) -> String {
    let url = url.trim_end_matches('/');
    let last = url.rsplit('/').next().unwrap_or(url);
    last.trim_end_matches(".git").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ManifestParser;

    const SAMPLE: &str = r#"packages: ./
          ./lib

source-repository-package
  type: git
  location: https://github.com/someone/something.git
  tag: abc123

source-repository-package
  type: git
  location: https://github.com/another/pkg.git
  tag: v2.0.0

constraints: text ==1.2.4.0,
             bytestring >=0.11
"#;

    #[test]
    fn test_parse_cabal_project() {
        let m = CabalProjectParser.parse(SAMPLE).unwrap();
        assert_eq!(m.ecosystem, "cabal");
        assert!(m.name.is_none());
        assert!(m.version.is_none());

        let names: Vec<&str> = m.dependencies.iter().map(|d| d.name.as_str()).collect();
        assert!(names.contains(&"something"), "{names:?}");
        assert!(names.contains(&"pkg"), "{names:?}");

        let something = m
            .dependencies
            .iter()
            .find(|d| d.name == "something")
            .unwrap();
        assert_eq!(something.version_req.as_deref(), Some("abc123"));
        assert_eq!(something.kind, DepKind::Normal);

        let pkg = m.dependencies.iter().find(|d| d.name == "pkg").unwrap();
        assert_eq!(pkg.version_req.as_deref(), Some("v2.0.0"));
    }

    #[test]
    fn test_no_source_repos() {
        let content = r#"packages: ./

constraints: base >=4.14
"#;
        let m = CabalProjectParser.parse(content).unwrap();
        assert!(m.dependencies.is_empty());
    }

    #[test]
    fn test_derive_name_from_url() {
        assert_eq!(
            derive_name_from_url("https://github.com/foo/bar.git"),
            "bar"
        );
        assert_eq!(derive_name_from_url("https://github.com/foo/bar"), "bar");
        assert_eq!(derive_name_from_url("https://github.com/foo/bar/"), "bar");
    }
}
