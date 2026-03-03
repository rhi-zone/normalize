//! Heuristic parser for `flake.nix` files (Nix).
//!
//! Full Nix expression evaluation is not feasible. We use line-pattern matching
//! to extract `inputs.<name>.url = "..."` declarations.
//!
//! Version requirements are NOT available — only dep names and their source URLs.
//! `version_req` is always `None`; the URL is stored in a separate `url` field
//! when needed. For `DeclaredDep`, the URL is omitted (it doesn't fit the schema).

use crate::{DeclaredDep, DepKind, ManifestError, ManifestParser, ParsedManifest};

/// Heuristic parser for `flake.nix` files.
pub struct FlakeParser;

impl ManifestParser for FlakeParser {
    fn filename(&self) -> &'static str {
        "flake.nix"
    }

    fn parse(&self, content: &str) -> Result<ParsedManifest, ManifestError> {
        let mut deps = Vec::new();
        let mut seen = std::collections::HashSet::new();
        let mut in_inputs_block = false;
        // Depth of `{` braces seen after entering the inputs block.
        // We exit when this returns to 0 (the `}` closing the inputs block itself).
        let mut inputs_depth: i32 = 0;

        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }

            // Detect `inputs = {` block
            if !in_inputs_block
                && trimmed.starts_with("inputs")
                && trimmed.contains('=')
                && trimmed.contains('{')
            {
                in_inputs_block = true;
                inputs_depth = 1; // the opening `{` of inputs = { ... }
                continue;
            }

            if in_inputs_block {
                // Track inner brace depth so we don't exit on sub-object `};`
                for ch in trimmed.chars() {
                    match ch {
                        '{' => inputs_depth += 1,
                        '}' => {
                            inputs_depth -= 1;
                            if inputs_depth <= 0 {
                                in_inputs_block = false;
                                break;
                            }
                        }
                        _ => {}
                    }
                }
                if !in_inputs_block {
                    continue;
                }
            }

            // Two patterns to find input names:
            //
            // 1. `inputs.<name>.url = "..."` (flat / inline with outputs attribute set)
            // 2. Inside `inputs = { ... }`: `<name>.url = "..."` or `<name> = { url = ...; }`
            let input_name = if let Some(rest) = trimmed.strip_prefix("inputs.") {
                // Pattern 1
                let name_end = rest.find(['.', ' ', '=', '{']).unwrap_or(rest.len());
                let n = rest[..name_end].trim().to_string();
                if rest.contains(".follows") {
                    continue; // skip follows declarations
                }
                n
            } else if in_inputs_block {
                // Pattern 2 — line inside inputs block like `nixpkgs.url = "..."` or `crane = {`
                // Skip lines that are clearly not input names (opening/closing braces, etc.)
                if trimmed == "{"
                    || trimmed == "};"
                    || trimmed == "}"
                    || trimmed.starts_with("url")
                    || trimmed.starts_with("inputs.")
                    || trimmed.starts_with("description")
                {
                    continue;
                }
                // Extract name: first identifier before `.` or `=` or `{` or space
                let name_end = trimmed.find(['.', ' ', '=', '{']).unwrap_or(trimmed.len());
                let n = trimmed[..name_end].trim().to_string();
                // Skip `.follows` lines
                if trimmed.contains(".follows") {
                    continue;
                }
                n
            } else {
                continue;
            };

            if input_name.is_empty() || input_name == "nixpkgs" {
                continue;
            }

            if seen.insert(input_name.clone()) {
                deps.push(DeclaredDep {
                    name: input_name,
                    version_req: None, // Nix flakes don't have semver constraints
                    kind: DepKind::Normal,
                });
            }
        }

        Ok(ParsedManifest {
            ecosystem: "nix",
            name: None,
            version: None,
            dependencies: deps,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ManifestParser;

    #[test]
    fn test_parse_flake_nix() {
        let content = r#"{
  description = "My Nix flake";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    crane.url = "github:ipetkov/crane";
  };

  outputs = { self, nixpkgs, flake-utils, rust-overlay, crane, ... }: {};
}
"#;
        let m = FlakeParser.parse(content).unwrap();
        assert_eq!(m.ecosystem, "nix");

        // nixpkgs is filtered
        assert!(!m.dependencies.iter().any(|d| d.name == "nixpkgs"));

        let names: Vec<&str> = m.dependencies.iter().map(|d| d.name.as_str()).collect();
        assert!(names.contains(&"flake-utils"));
        assert!(names.contains(&"rust-overlay"));
        assert!(names.contains(&"crane"));

        // follows entries should not create separate deps
        assert_eq!(m.dependencies.len(), 3);
    }
}
