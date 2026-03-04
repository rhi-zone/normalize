//! Parser for `build.zig.zon` files (Zig).
//!
//! ZON (Zig Object Notation) is a subset of Zig syntax used for package
//! manifests. We use heuristic line-pattern matching to extract `.name`,
//! `.version`, and `.dependencies` without a full Zig parser.

use crate::{DeclaredDep, DepKind, ManifestError, ManifestParser, ParsedManifest};

/// Parser for `build.zig.zon` files.
pub struct ZigZonParser;

impl ManifestParser for ZigZonParser {
    fn filename(&self) -> &'static str {
        "build.zig.zon"
    }

    fn parse(&self, content: &str) -> Result<ParsedManifest, ManifestError> {
        let mut name: Option<String> = None;
        let mut version: Option<String> = None;
        let mut deps: Vec<DeclaredDep> = Vec::new();

        // Track parser state.
        // We care about three depth levels:
        //   top-level `.{ ... }` → depth 1
        //   `.dependencies = .{ ... }` → depth 2
        //   individual dep `.depname = .{ ... }` → depth 3
        #[derive(PartialEq)]
        enum State {
            TopLevel,
            InDeps,
            InDepEntry,
        }

        let mut state = State::TopLevel;
        let mut depth: usize = 0;
        let mut deps_depth: usize = 0;
        let mut dep_entry_depth: usize = 0;
        let mut current_dep_name: Option<String> = None;

        for line in content.lines() {
            let trimmed = line.trim();

            // Skip comments
            if trimmed.starts_with("//") || trimmed.is_empty() {
                continue;
            }

            // Count brace changes on this line.
            let opens = trimmed.chars().filter(|&c| c == '{').count();
            let closes = trimmed.chars().filter(|&c| c == '}').count();

            // Extract .name and .version at top level (depth 1).
            if state == State::TopLevel && depth <= 1 {
                if let Some(v) = extract_field_string(trimmed, ".name")
                    && name.is_none()
                {
                    name = Some(v);
                }
                if let Some(v) = extract_field_string(trimmed, ".version")
                    && version.is_none()
                {
                    version = Some(v);
                }
            }

            // Detect entry into .dependencies = .{
            if state == State::TopLevel
                && trimmed.contains(".dependencies")
                && trimmed.contains('=')
                && opens > 0
            {
                state = State::InDeps;
                deps_depth = depth + opens - closes;
                depth = depth + opens - closes;
                continue;
            }

            // Inside dependencies block, detect individual dep entries: .name = .{
            if state == State::InDeps {
                // Check for leaving the deps block.
                let new_depth = depth + opens - closes;
                if new_depth < deps_depth {
                    state = State::TopLevel;
                    depth = new_depth;
                    continue;
                }

                // Detect a dep entry: line like `.depname = .{`
                if opens > 0
                    && trimmed.starts_with('.')
                    && trimmed.contains('=')
                    && let Some(dep_name) = extract_zon_key(trimmed)
                {
                    state = State::InDepEntry;
                    current_dep_name = Some(dep_name);
                    dep_entry_depth = new_depth;
                    depth = new_depth;
                    continue;
                }

                depth = new_depth;
                continue;
            }

            // Inside a single dep entry.
            if state == State::InDepEntry {
                let new_depth = depth + opens - closes;
                if new_depth < dep_entry_depth {
                    // Leaving this dep entry; emit it.
                    if let Some(dep_name) = current_dep_name.take() {
                        deps.push(DeclaredDep {
                            name: dep_name,
                            version_req: None,
                            kind: DepKind::Normal,
                        });
                    }
                    // Are we back in deps or fully out?
                    if new_depth < deps_depth {
                        state = State::TopLevel;
                    } else {
                        state = State::InDeps;
                    }
                    depth = new_depth;
                    continue;
                }
                depth = new_depth;
                continue;
            }

            depth = (depth + opens).saturating_sub(closes);
        }

        // Handle unclosed final dep entry (file ends without closing brace).
        if state == State::InDepEntry
            && let Some(dep_name) = current_dep_name.take()
        {
            deps.push(DeclaredDep {
                name: dep_name,
                version_req: None,
                kind: DepKind::Normal,
            });
        }

        Ok(ParsedManifest {
            ecosystem: "zig",
            name,
            version,
            dependencies: deps,
        })
    }
}

/// Extract the value from `.field = "value"` or `.field = "value",`.
fn extract_field_string(line: &str, field: &str) -> Option<String> {
    let rest = line.strip_prefix(field)?.trim();
    let rest = rest.strip_prefix('=')?.trim();
    let rest = rest.strip_prefix('"')?;
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

/// Extract the key name from `.keyname = .{` or `.keyname = .{`.
fn extract_zon_key(line: &str) -> Option<String> {
    let rest = line.strip_prefix('.')?;
    // key name ends at whitespace or `=`
    let end = rest.find(|c: char| c.is_whitespace() || c == '=')?;
    let key = rest[..end].trim();
    if key.is_empty() {
        None
    } else {
        Some(key.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ManifestParser;

    const SAMPLE: &str = r#".{
    .name = "my-project",
    .version = "0.12.0",
    .minimum_zig_version = "0.12.0",
    .dependencies = .{
        .zap = .{
            .url = "https://github.com/zigzap/zap/archive/refs/tags/v0.2.0.tar.gz",
            .hash = "122059d35a68afb4f5e59b52fdc63be4c09ee07f72bf7c7abaab46c5ebe8c39e8f",
        },
        .known_folders = .{
            .url = "https://github.com/ziglibs/known-folders/archive/fa75e1bc672952efa0cf06160bbd942b47f6d59b.tar.gz",
            .hash = "122048992d",
        },
    },
}
"#;

    #[test]
    fn test_parse_zig_zon() {
        let m = ZigZonParser.parse(SAMPLE).unwrap();
        assert_eq!(m.ecosystem, "zig");
        assert_eq!(m.name.as_deref(), Some("my-project"));
        assert_eq!(m.version.as_deref(), Some("0.12.0"));

        let names: Vec<&str> = m.dependencies.iter().map(|d| d.name.as_str()).collect();
        assert!(names.contains(&"zap"), "{names:?}");
        assert!(names.contains(&"known_folders"), "{names:?}");
        assert_eq!(m.dependencies.len(), 2);
        assert!(m.dependencies.iter().all(|d| d.version_req.is_none()));
        assert!(m.dependencies.iter().all(|d| d.kind == DepKind::Normal));
    }

    #[test]
    fn test_no_deps() {
        let content = r#".{
    .name = "simple",
    .version = "0.1.0",
}
"#;
        let m = ZigZonParser.parse(content).unwrap();
        assert_eq!(m.name.as_deref(), Some("simple"));
        assert_eq!(m.version.as_deref(), Some("0.1.0"));
        assert!(m.dependencies.is_empty());
    }
}
