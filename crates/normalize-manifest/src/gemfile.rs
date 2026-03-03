//! Parser for `Gemfile` files (Ruby/Bundler).
//!
//! Extracts `gem` declarations:
//! - `gem "name"`
//! - `gem "name", "~> 1.0"`
//! - `gem "name", ">= 1.0", "< 2.0"` (multiple constraints joined)
//! - `gem "name", group: :development` → DepKind::Dev

use crate::{DeclaredDep, DepKind, ManifestError, ManifestParser, ParsedManifest};

/// Parser for `Gemfile` files.
pub struct GemfileParser;

impl ManifestParser for GemfileParser {
    fn filename(&self) -> &'static str {
        "Gemfile"
    }

    fn parse(&self, content: &str) -> Result<ParsedManifest, ManifestError> {
        let mut deps = Vec::new();
        let mut current_group: Option<DepKind> = None;

        for line in content.lines() {
            let trimmed = line.trim();

            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }

            // group :development, :test do ... end
            if trimmed.starts_with("group") && trimmed.ends_with("do") {
                current_group = Some(gemfile_group_kind(trimmed));
                continue;
            }
            if trimmed == "end" {
                current_group = None;
                continue;
            }

            if (trimmed.starts_with("gem ") || trimmed.starts_with("gem\t"))
                && let Some(dep) = parse_gem_line(trimmed, current_group)
            {
                deps.push(dep);
            }
        }

        Ok(ParsedManifest {
            ecosystem: "bundler",
            name: None,
            version: None,
            dependencies: deps,
        })
    }
}

fn gemfile_group_kind(line: &str) -> DepKind {
    if line.contains(":development") || line.contains(":dev") || line.contains(":test") {
        DepKind::Dev
    } else {
        DepKind::Optional
    }
}

fn parse_gem_line(line: &str, group_override: Option<DepKind>) -> Option<DeclaredDep> {
    // Strip leading `gem`
    let rest = line.trim_start_matches("gem").trim();

    // Collect all quoted tokens and keyword args
    let mut quoted: Vec<String> = Vec::new();
    let mut kind = DepKind::Normal;

    // Detect inline group: keyword arg  `group: :development`
    if rest.contains("group:")
        && (rest.contains(":development") || rest.contains(":dev") || rest.contains(":test"))
    {
        kind = DepKind::Dev;
    }

    // Extract all single- or double-quoted strings
    let mut chars = rest.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '"' || ch == '\'' {
            let mut s = String::new();
            for inner in chars.by_ref() {
                if inner == ch {
                    break;
                }
                s.push(inner);
            }
            quoted.push(s);
        }
    }

    if quoted.is_empty() {
        return None;
    }

    let name = quoted[0].clone();
    if name.is_empty() {
        return None;
    }

    // Remaining quoted strings that look like version constraints
    let version_parts: Vec<&str> = quoted[1..]
        .iter()
        .filter(|s| {
            s.starts_with('~')
                || s.starts_with('>')
                || s.starts_with('<')
                || s.starts_with('=')
                || s.chars().next().is_some_and(|c| c.is_ascii_digit())
        })
        .map(|s| s.as_str())
        .collect();

    let version_req = if version_parts.is_empty() {
        None
    } else {
        Some(version_parts.join(", "))
    };

    let final_kind = group_override.unwrap_or(kind);

    Some(DeclaredDep {
        name,
        version_req,
        kind: final_kind,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ManifestParser;

    #[test]
    fn test_parse_gemfile() {
        let content = r#"source "https://rubygems.org"

gem "rails", "~> 7.0"
gem "pg", ">= 0.18", "< 2.0"
gem "puma"

group :development, :test do
  gem "rspec-rails"
  gem "factory_bot_rails"
end

gem "capistrano", group: :development
"#;
        let m = GemfileParser.parse(content).unwrap();
        assert_eq!(m.ecosystem, "bundler");

        let rails = m.dependencies.iter().find(|d| d.name == "rails").unwrap();
        assert_eq!(rails.version_req.as_deref(), Some("~> 7.0"));
        assert_eq!(rails.kind, DepKind::Normal);

        let pg = m.dependencies.iter().find(|d| d.name == "pg").unwrap();
        assert_eq!(pg.version_req.as_deref(), Some(">= 0.18, < 2.0"));

        let puma = m.dependencies.iter().find(|d| d.name == "puma").unwrap();
        assert!(puma.version_req.is_none());

        let rspec = m
            .dependencies
            .iter()
            .find(|d| d.name == "rspec-rails")
            .unwrap();
        assert_eq!(rspec.kind, DepKind::Dev);

        let cap = m
            .dependencies
            .iter()
            .find(|d| d.name == "capistrano")
            .unwrap();
        assert_eq!(cap.kind, DepKind::Dev);
    }
}
