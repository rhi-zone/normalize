//! Parser for `setup.cfg` files (Python / setuptools).
//!
//! Handles:
//! - `[metadata]` section: `name`, `version`
//! - `[options]` section: `install_requires` (multi-line list)
//! - `[options.extras_require]` section: optional dep groups → `DepKind::Optional`

use crate::pip::parse_pip_requirement;
use crate::{DeclaredDep, DepKind, ManifestError, ManifestParser, ParsedManifest};

/// Parser for `setup.cfg` files.
pub struct SetupCfgParser;

impl ManifestParser for SetupCfgParser {
    fn filename(&self) -> &'static str {
        "setup.cfg"
    }

    fn parse(&self, content: &str) -> Result<ParsedManifest, ManifestError> {
        let mut name = None;
        let mut version = None;
        let mut deps: Vec<DeclaredDep> = Vec::new();

        #[derive(PartialEq)]
        enum Section {
            Other,
            Metadata,
            Options,
            Extras,
        }

        let mut section = Section::Other;
        let mut collecting_requires = false;
        let mut current_extras_kind = DepKind::Optional;

        for line in content.lines() {
            let trimmed = line.trim();

            // Skip comments and blank lines (but blank lines reset continuation)
            if trimmed.is_empty() {
                collecting_requires = false;
                continue;
            }
            if trimmed.starts_with('#') || trimmed.starts_with(';') {
                continue;
            }

            // Section header
            if trimmed.starts_with('[') && trimmed.ends_with(']') {
                collecting_requires = false;
                let sec = &trimmed[1..trimmed.len() - 1];
                section = match sec {
                    "metadata" => Section::Metadata,
                    "options" => Section::Options,
                    s if s.starts_with("options.extras_require") => {
                        current_extras_kind = DepKind::Optional;
                        Section::Extras
                    }
                    _ => Section::Other,
                };
                continue;
            }

            // Continuation line (indented) — belongs to previous key
            if line.starts_with([' ', '\t']) && collecting_requires {
                let kind = if section == Section::Extras {
                    current_extras_kind
                } else {
                    DepKind::Normal
                };
                if let Some(dep) = parse_pip_requirement(trimmed) {
                    deps.push(DeclaredDep { kind, ..dep });
                }
                continue;
            }

            // Key = value
            if let Some(eq_idx) = trimmed.find('=') {
                collecting_requires = false;
                let key = trimmed[..eq_idx].trim();
                let value = trimmed[eq_idx + 1..].trim();

                match section {
                    Section::Metadata => match key {
                        "name" => name = Some(value.to_string()),
                        "version" => version = Some(value.to_string()),
                        _ => {}
                    },
                    Section::Options => {
                        if key == "install_requires" {
                            collecting_requires = true;
                            // Inline value (uncommon but possible)
                            if !value.is_empty()
                                && let Some(dep) = parse_pip_requirement(value)
                            {
                                deps.push(dep);
                            }
                        }
                    }
                    Section::Extras => {
                        // extras_require section: each key is a group name (e.g. `dev =`)
                        // The value may be inline or multi-line
                        collecting_requires = true;
                        if !value.is_empty()
                            && let Some(dep) = parse_pip_requirement(value)
                        {
                            deps.push(DeclaredDep {
                                kind: DepKind::Optional,
                                ..dep
                            });
                        }
                    }
                    Section::Other => {}
                }
            }
        }

        Ok(ParsedManifest {
            ecosystem: "python",
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
    fn test_parse_setup_cfg() {
        let content = r#"[metadata]
name = my-package
version = 1.2.3

[options]
install_requires =
    requests>=2.28
    flask>=2.0
    numpy

[options.extras_require]
dev =
    pytest
    black
"#;
        let m = SetupCfgParser.parse(content).unwrap();
        assert_eq!(m.ecosystem, "python");
        assert_eq!(m.name.as_deref(), Some("my-package"));
        assert_eq!(m.version.as_deref(), Some("1.2.3"));

        let normal: Vec<_> = m
            .dependencies
            .iter()
            .filter(|d| d.kind == DepKind::Normal)
            .collect();
        assert_eq!(normal.len(), 3);
        assert!(normal.iter().any(|d| d.name == "requests"));
        assert!(normal.iter().any(|d| d.name == "flask"));
        assert!(normal.iter().any(|d| d.name == "numpy"));

        let optional: Vec<_> = m
            .dependencies
            .iter()
            .filter(|d| d.kind == DepKind::Optional)
            .collect();
        assert_eq!(optional.len(), 2);
    }
}
