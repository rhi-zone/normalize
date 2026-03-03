//! Parser for `mix.exs` files (Elixir/Hex).
//!
//! Extracts `{:pkg, "~> 1.0"}` tuples from the `deps/0` function.
//! Detects dev/test deps via `only: :dev`, `only: :test`, or `only: [:dev, :test]`.

use crate::{DeclaredDep, DepKind, ManifestError, ManifestParser, ParsedManifest};

/// Parser for `mix.exs` files.
pub struct MixExsParser;

impl ManifestParser for MixExsParser {
    fn filename(&self) -> &'static str {
        "mix.exs"
    }

    fn parse(&self, content: &str) -> Result<ParsedManifest, ManifestError> {
        let mut name = None;
        let mut version = None;
        let mut deps = Vec::new();
        let mut in_deps_fn = false;
        let mut brace_depth: i32 = 0;

        for line in content.lines() {
            let trimmed = line.trim();

            // Detect app name and version from project/0
            if trimmed.starts_with("app:")
                && trimmed.contains(':')
                && let Some(app_name) = extract_atom_or_string(trimmed, "app:")
            {
                name = Some(app_name);
            }
            if trimmed.starts_with("version:")
                && let Some(ver) = extract_atom_or_string(trimmed, "version:")
            {
                version = Some(ver);
            }

            // Detect start of deps/0 function
            if trimmed.starts_with("defp deps") || trimmed == "def deps do" {
                in_deps_fn = true;
                brace_depth = 0;
            }

            if in_deps_fn {
                for ch in trimmed.chars() {
                    match ch {
                        '[' | '{' => brace_depth += 1,
                        ']' | '}' => {
                            brace_depth -= 1;
                            if brace_depth < 0 {
                                brace_depth = 0;
                            }
                        }
                        _ => {}
                    }
                }

                // Parse dep tuple: {:pkg_name, "~> 1.0"} or {:pkg, git: "...", only: :dev}
                if trimmed.contains('{')
                    && trimmed.contains(':')
                    && let Some(dep) = parse_mix_dep(trimmed)
                {
                    deps.push(dep);
                }

                // Exit deps function when indentation returns to base (simplified: look for `end`)
                if trimmed == "end" && brace_depth == 0 {
                    in_deps_fn = false;
                }
            }
        }

        Ok(ParsedManifest {
            ecosystem: "hex",
            name,
            version,
            dependencies: deps,
        })
    }
}

fn extract_atom_or_string(line: &str, prefix: &str) -> Option<String> {
    let rest = line.split_once(prefix)?.1.trim();
    // Atom: :my_app  or  string: "my_app"
    if let Some(atom_rest) = rest.strip_prefix(':') {
        let atom = atom_rest
            .split(|c: char| !c.is_alphanumeric() && c != '_')
            .next()?;
        return Some(atom.to_string());
    }
    if let Some(inner) = rest.strip_prefix('"') {
        let end = inner.find('"')?;
        return Some(inner[..end].to_string());
    }
    None
}

fn parse_mix_dep(line: &str) -> Option<DeclaredDep> {
    // Find `{:atom_name` opening
    let brace_start = line.find('{')? + 1;
    let inner = line[brace_start..].trim();

    if !inner.starts_with(':') {
        return None;
    }
    let name_end = inner[1..].find(|c: char| !c.is_alphanumeric() && c != '_')? + 1;
    let name = inner[1..name_end].to_string();
    if name.is_empty() {
        return None;
    }

    // Determine kind from `only:` annotation
    let kind = if line.contains("only: :dev")
        || line.contains("only: :test")
        || line.contains("only: [:dev")
        || line.contains("only: [:test")
    {
        DepKind::Dev
    } else {
        DepKind::Normal
    };

    // Extract version string if present: first `"..."` after the name
    let after_name = &inner[name_end..];
    let version_req = if let Some(q_start) = after_name.find('"') {
        let rest = &after_name[q_start + 1..];
        rest.find('"').map(|q_end| rest[..q_end].to_string())
    } else {
        None
    };

    Some(DeclaredDep {
        name,
        version_req,
        kind,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ManifestParser;

    #[test]
    fn test_parse_mix_exs() {
        let content = r#"defmodule MyApp.MixProject do
  use Mix.Project

  def project do
    [
      app: :my_app,
      version: "0.1.0",
      elixir: "~> 1.14",
      deps: deps()
    ]
  end

  defp deps do
    [
      {:phoenix, "~> 1.7"},
      {:ecto_sql, "~> 3.10"},
      {:postgrex, ">= 0.0.0"},
      {:credo, "~> 1.7", only: [:dev, :test], runtime: false},
      {:ex_doc, "~> 0.27", only: :dev, runtime: false}
    ]
  end
end
"#;
        let m = MixExsParser.parse(content).unwrap();
        assert_eq!(m.ecosystem, "hex");
        assert_eq!(m.name.as_deref(), Some("my_app"));
        assert_eq!(m.version.as_deref(), Some("0.1.0"));

        let phoenix = m.dependencies.iter().find(|d| d.name == "phoenix").unwrap();
        assert_eq!(phoenix.version_req.as_deref(), Some("~> 1.7"));
        assert_eq!(phoenix.kind, DepKind::Normal);

        let credo = m.dependencies.iter().find(|d| d.name == "credo").unwrap();
        assert_eq!(credo.kind, DepKind::Dev);

        let ex_doc = m.dependencies.iter().find(|d| d.name == "ex_doc").unwrap();
        assert_eq!(ex_doc.kind, DepKind::Dev);
    }
}
