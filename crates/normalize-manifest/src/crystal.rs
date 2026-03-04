//! Parser for `shard.yml` files (Crystal/Shards).
//!
//! Heuristic YAML parsing by indentation level:
//! - `dependencies:` section → `DepKind::Normal`
//! - `development_dependencies:` section → `DepKind::Dev`
//! - Dep name: 2-space indented key
//! - `version:` sub-key at 4+ spaces → version requirement

use crate::{DeclaredDep, DepKind, ManifestError, ManifestParser, ParsedManifest};

/// Parser for `shard.yml` files (Crystal Shards).
pub struct CrystalShardsParser;

impl ManifestParser for CrystalShardsParser {
    fn filename(&self) -> &'static str {
        "shard.yml"
    }

    fn parse(&self, content: &str) -> Result<ParsedManifest, ManifestError> {
        let mut name: Option<String> = None;
        let mut version: Option<String> = None;
        let mut deps: Vec<DeclaredDep> = Vec::new();

        #[derive(PartialEq)]
        enum Section {
            None,
            Dependencies,
            DevDependencies,
        }

        let mut section = Section::None;
        let mut current_dep_name: Option<String> = None;
        let mut current_dep_kind = DepKind::Normal;
        let mut current_dep_version: Option<String> = None;

        for line in content.lines() {
            // Skip blank lines and comments
            let stripped = line.trim_end();
            if stripped.trim().is_empty() || stripped.trim().starts_with('#') {
                continue;
            }

            // Count leading spaces
            let leading_spaces = stripped.len() - stripped.trim_start().len();
            let trimmed = stripped.trim();

            if leading_spaces == 0 {
                // Flush any pending dep
                flush_dep(
                    &mut current_dep_name,
                    &mut current_dep_version,
                    current_dep_kind,
                    &mut deps,
                );

                if trimmed == "dependencies:" {
                    section = Section::Dependencies;
                } else if trimmed == "development_dependencies:" {
                    section = Section::DevDependencies;
                } else {
                    // Top-level key: value
                    if let Some((key, val)) = split_key_value(trimmed) {
                        match key {
                            "name" => name = Some(val.to_string()),
                            "version" => version = Some(val.to_string()),
                            _ => {}
                        }
                    }
                    section = Section::None;
                }
            } else if leading_spaces == 2
                && (section == Section::Dependencies || section == Section::DevDependencies)
            {
                // Flush previous dep
                flush_dep(
                    &mut current_dep_name,
                    &mut current_dep_version,
                    current_dep_kind,
                    &mut deps,
                );

                // A dep name entry: "  pkgname:"
                if let Some(dep_name) = trimmed.strip_suffix(':')
                    && !dep_name.starts_with('#')
                {
                    current_dep_name = Some(dep_name.to_string());
                    current_dep_kind = if section == Section::DevDependencies {
                        DepKind::Dev
                    } else {
                        DepKind::Normal
                    };
                    current_dep_version = None;
                }
            } else if leading_spaces >= 4 && current_dep_name.is_some() {
                // Sub-key of a dep
                if let Some((key, val)) = split_key_value(trimmed)
                    && key == "version"
                    && !val.is_empty()
                {
                    current_dep_version = Some(val.to_string());
                }
            }
        }

        // Flush last dep
        flush_dep(
            &mut current_dep_name,
            &mut current_dep_version,
            current_dep_kind,
            &mut deps,
        );

        Ok(ParsedManifest {
            ecosystem: "shards",
            name,
            version,
            dependencies: deps,
        })
    }
}

fn flush_dep(
    name: &mut Option<String>,
    version: &mut Option<String>,
    kind: DepKind,
    deps: &mut Vec<DeclaredDep>,
) {
    if let Some(n) = name.take() {
        deps.push(DeclaredDep {
            name: n,
            version_req: version.take(),
            kind,
        });
    }
    *version = None;
}

/// Split `key: value` or `key:` → `(key, value)`. Strips surrounding quotes.
fn split_key_value(s: &str) -> Option<(&str, &str)> {
    let colon = s.find(':')?;
    let key = s[..colon].trim();
    let raw_val = s[colon + 1..].trim();
    // Strip surrounding quotes from value
    let val = raw_val
        .trim_start_matches('"')
        .trim_end_matches('"')
        .trim_start_matches('\'')
        .trim_end_matches('\'');
    Some((key, val))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ManifestParser;

    #[test]
    fn test_parse_shard_yml() {
        let content = r#"name: my_project
version: 0.1.0

authors:
  - Alice <alice@example.com>

dependencies:
  mysql:
    github: crystal-lang/crystal-mysql
    version: ~> 0.13.0
  kemal:
    github: kemalcr/kemal
    version: ~> 1.3
  redis:
    github: stefanwille/crystal-redis

development_dependencies:
  webmock:
    github: manastech/webmock.cr
    version: ~> 0.3
"#;
        let m = CrystalShardsParser.parse(content).unwrap();
        assert_eq!(m.ecosystem, "shards");
        assert_eq!(m.name.as_deref(), Some("my_project"));
        assert_eq!(m.version.as_deref(), Some("0.1.0"));

        let mysql = m.dependencies.iter().find(|d| d.name == "mysql").unwrap();
        assert_eq!(mysql.kind, DepKind::Normal);
        assert_eq!(mysql.version_req.as_deref(), Some("~> 0.13.0"));

        let kemal = m.dependencies.iter().find(|d| d.name == "kemal").unwrap();
        assert_eq!(kemal.version_req.as_deref(), Some("~> 1.3"));

        let redis = m.dependencies.iter().find(|d| d.name == "redis").unwrap();
        assert!(redis.version_req.is_none());

        let webmock = m.dependencies.iter().find(|d| d.name == "webmock").unwrap();
        assert_eq!(webmock.kind, DepKind::Dev);
        assert_eq!(webmock.version_req.as_deref(), Some("~> 0.3"));
    }

    #[test]
    fn test_no_dev_deps() {
        let content = r#"name: minimal
version: 0.2.0

dependencies:
  lucky:
    github: luckyframework/lucky
    version: ~> 1.0.0
"#;
        let m = CrystalShardsParser.parse(content).unwrap();
        assert_eq!(m.dependencies.len(), 1);
        assert_eq!(m.dependencies[0].name, "lucky");
        assert_eq!(m.dependencies[0].kind, DepKind::Normal);
    }
}
