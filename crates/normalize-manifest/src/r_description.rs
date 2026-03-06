//! Parser for R `DESCRIPTION` files (CRAN/DCF format).
//!
//! DCF (Debian Control File) format: `Key: value` with continuation lines
//! starting with whitespace. Extracts:
//! - `Imports:` → `DepKind::Normal`
//! - `Depends:` → `DepKind::Normal` (skips the `R` runtime entry)
//! - `Suggests:` → `DepKind::Dev`
//! - `LinkingTo:` → `DepKind::Build`
//!
//! Version constraints like `dplyr (>= 1.0.0)` are split into name + version_req.

use crate::{DeclaredDep, DepKind, ManifestError, ManifestParser, ParsedManifest};

/// Parser for R `DESCRIPTION` files.
pub struct RDescriptionParser;

impl ManifestParser for RDescriptionParser {
    fn filename(&self) -> &'static str {
        "DESCRIPTION"
    }

    fn parse(&self, content: &str) -> Result<ParsedManifest, ManifestError> {
        let mut name: Option<String> = None;
        let mut version: Option<String> = None;
        let mut deps: Vec<DeclaredDep> = Vec::new();

        #[derive(Clone, Copy, PartialEq)]
        enum Field {
            None,
            Imports,
            Suggests,
            Depends,
            LinkingTo,
        }

        let mut current_field = Field::None;
        // Accumulated value text across continuation lines
        let mut field_value = String::new();

        let flush = |field: Field, value: &str, deps: &mut Vec<DeclaredDep>| {
            if field == Field::None || value.is_empty() {
                return;
            }
            let kind = match field {
                Field::Imports => DepKind::Normal,
                Field::Depends => DepKind::Normal,
                Field::Suggests => DepKind::Dev,
                Field::LinkingTo => DepKind::Build,
                Field::None => return,
            };
            for entry in value.split(',') {
                let entry = entry.trim();
                if entry.is_empty() {
                    continue;
                }
                let dep_entry = parse_r_dep_entry(entry);
                if dep_entry.pkg_name.is_empty() || dep_entry.pkg_name == "R" {
                    continue;
                }
                deps.push(DeclaredDep {
                    name: dep_entry.pkg_name,
                    version_req: dep_entry.version_req,
                    kind,
                });
            }
        };

        for line in content.lines() {
            // Continuation line: starts with whitespace
            if line.starts_with(' ') || line.starts_with('\t') {
                if current_field != Field::None {
                    if !field_value.is_empty() {
                        field_value.push(',');
                    }
                    field_value.push_str(line.trim());
                }
                continue;
            }

            // New field: flush previous
            flush(current_field, &field_value, &mut deps);
            field_value.clear();
            current_field = Field::None;

            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            if let Some(colon) = trimmed.find(':') {
                let key = trimmed[..colon].trim();
                let val = trimmed[colon + 1..].trim();

                match key {
                    "Package" => name = Some(val.to_string()),
                    "Version" => version = Some(val.to_string()),
                    "Imports" => {
                        current_field = Field::Imports;
                        field_value = val.to_string();
                    }
                    "Suggests" => {
                        current_field = Field::Suggests;
                        field_value = val.to_string();
                    }
                    "Depends" => {
                        current_field = Field::Depends;
                        field_value = val.to_string();
                    }
                    "LinkingTo" => {
                        current_field = Field::LinkingTo;
                        field_value = val.to_string();
                    }
                    _ => {}
                }
            }
        }

        // Flush last field
        flush(current_field, &field_value, &mut deps);

        Ok(ParsedManifest {
            ecosystem: "cran",
            name,
            version,
            dependencies: deps,
        })
    }
}

struct RDepEntry {
    pkg_name: String,
    version_req: Option<String>,
}

/// Parse `dplyr (>= 1.0.0)` → name + optional version.
/// Also handles bare `ggplot2` → name with no version.
fn parse_r_dep_entry(s: &str) -> RDepEntry {
    if let Some(paren) = s.find('(') {
        let pkg_name = s[..paren].trim().to_string();
        let rest = s[paren + 1..].trim();
        let ver = rest.trim_end_matches(')').trim().to_string();
        let version_req = if ver.is_empty() { None } else { Some(ver) };
        RDepEntry {
            pkg_name,
            version_req,
        }
    } else {
        RDepEntry {
            pkg_name: s.trim().to_string(),
            version_req: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ManifestParser;

    #[test]
    fn test_parse_r_description() {
        let content = r#"Package: mypackage
Title: My R Package
Version: 1.0.0
Authors@R: person("Alice", "Smith", role = c("aut", "cre"))
Description: Does things.
Imports:
    dplyr (>= 1.0.0),
    ggplot2,
    stringr (>= 1.4.0)
Suggests:
    testthat (>= 3.0.0),
    knitr,
    rmarkdown
Depends:
    R (>= 4.0.0)
LinkingTo:
    Rcpp
License: MIT
"#;
        let m = RDescriptionParser.parse(content).unwrap();
        assert_eq!(m.ecosystem, "cran");
        assert_eq!(m.name.as_deref(), Some("mypackage"));
        assert_eq!(m.version.as_deref(), Some("1.0.0"));

        let dplyr = m.dependencies.iter().find(|d| d.name == "dplyr").unwrap();
        assert_eq!(dplyr.kind, DepKind::Normal);
        assert_eq!(dplyr.version_req.as_deref(), Some(">= 1.0.0"));

        let ggplot = m.dependencies.iter().find(|d| d.name == "ggplot2").unwrap();
        assert_eq!(ggplot.kind, DepKind::Normal);
        assert!(ggplot.version_req.is_none());

        let testthat = m
            .dependencies
            .iter()
            .find(|d| d.name == "testthat")
            .unwrap();
        assert_eq!(testthat.kind, DepKind::Dev);
        assert_eq!(testthat.version_req.as_deref(), Some(">= 3.0.0"));

        let rcpp = m.dependencies.iter().find(|d| d.name == "Rcpp").unwrap();
        assert_eq!(rcpp.kind, DepKind::Build);

        // R itself should be skipped
        assert!(!m.dependencies.iter().any(|d| d.name == "R"));
    }

    #[test]
    fn test_inline_imports() {
        // Imports on same line as key (no continuation)
        let content = "Package: tiny\nVersion: 0.0.1\nImports: data.table, jsonlite\n";
        let m = RDescriptionParser.parse(content).unwrap();
        assert_eq!(m.dependencies.len(), 2);
        assert!(m.dependencies.iter().any(|d| d.name == "data.table"));
        assert!(m.dependencies.iter().any(|d| d.name == "jsonlite"));
    }
}
