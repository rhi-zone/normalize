//! Parser for `build.sbt` files (Scala/sbt).
//!
//! Extracts `libraryDependencies` declarations:
//! - `libraryDependencies += "org" %% "name" % "version"`
//! - `libraryDependencies += "org" % "name" % "version" % Test`
//! - `libraryDependencies ++= Seq(...)` multi-dep blocks

use crate::{DeclaredDep, DepKind, ManifestError, ManifestParser, ParsedManifest};

/// Parser for `build.sbt` files.
pub struct SbtParser;

impl ManifestParser for SbtParser {
    fn filename(&self) -> &'static str {
        "build.sbt"
    }

    fn parse(&self, content: &str) -> Result<ParsedManifest, ManifestError> {
        let mut name = None;
        let mut version = None;
        let mut deps = Vec::new();

        // Join continuation lines (lines ending with `\` or inside unclosed parens)
        // Simple approach: process line by line, accumulating multi-line blocks
        let mut accumulator = String::new();
        let mut paren_depth: i32 = 0;

        for line in content.lines() {
            let trimmed = line.trim();

            if trimmed.is_empty() || trimmed.starts_with("//") {
                if paren_depth == 0 && !accumulator.trim().is_empty() {
                    process_sbt_statement(accumulator.trim(), &mut name, &mut version, &mut deps);
                    accumulator.clear();
                }
                continue;
            }

            accumulator.push(' ');
            accumulator.push_str(trimmed);

            for ch in trimmed.chars() {
                match ch {
                    '(' => paren_depth += 1,
                    ')' => {
                        paren_depth -= 1;
                        if paren_depth < 0 {
                            paren_depth = 0;
                        }
                    }
                    _ => {}
                }
            }

            if paren_depth == 0 {
                process_sbt_statement(accumulator.trim(), &mut name, &mut version, &mut deps);
                accumulator.clear();
            }
        }
        if !accumulator.trim().is_empty() {
            process_sbt_statement(accumulator.trim(), &mut name, &mut version, &mut deps);
        }

        Ok(ParsedManifest {
            ecosystem: "sbt",
            name,
            version,
            dependencies: deps,
        })
    }
}

fn process_sbt_statement(
    stmt: &str,
    name: &mut Option<String>,
    version: &mut Option<String>,
    deps: &mut Vec<DeclaredDep>,
) {
    let stmt = stmt.trim();

    // name := "my-project"
    if stmt.starts_with("name") && stmt.contains(":=") {
        if let Some(val) = extract_sbt_string(stmt) {
            *name = Some(val);
        }
        return;
    }
    // version := "1.0.0"
    if stmt.starts_with("version") && stmt.contains(":=") {
        if let Some(val) = extract_sbt_string(stmt) {
            *version = Some(val);
        }
        return;
    }

    // libraryDependencies += ...  or  libraryDependencies ++= Seq(...)
    if !stmt.contains("libraryDependencies") {
        return;
    }

    // Extract all %% / % separated dep tuples from the statement
    // Each dep looks like: "org" %% "name" % "version"  optionally % Test/Provided/...
    parse_sbt_deps(stmt, deps);
}

fn parse_sbt_deps(stmt: &str, out: &mut Vec<DeclaredDep>) {
    // Find quoted strings in sequence — groups of 3 are a dep
    let strings = collect_quoted_strings(stmt);

    // Walk through finding triples separated by % operators
    // We'll look for sequences: org, artifact, version [, scope]
    let mut i = 0;
    while i + 2 < strings.len() {
        let org = &strings[i];
        let artifact = &strings[i + 1];
        let ver = &strings[i + 2];

        // Heuristic: artifact names are lowercase with hyphens/underscores, versions start with digit
        let name = format!("{}:{}", org, artifact);
        let version_req = Some(ver.clone());

        // Check for scope keyword after the version string (including its closing quote)
        let quoted_ver = format!("\"{}\"", ver);
        let rest_idx = stmt
            .find(quoted_ver.as_str())
            .map(|p| p + quoted_ver.len())
            .unwrap_or(stmt.len());
        let rest = stmt[rest_idx..].trim();
        let kind = if rest.starts_with('%') {
            let scope_part = rest.trim_start_matches('%').trim();
            if scope_part.starts_with("Test")
                || scope_part.starts_with('"') && scope_part.contains("test")
            {
                DepKind::Dev
            } else if scope_part.starts_with("Provided") {
                DepKind::Optional
            } else {
                DepKind::Normal
            }
        } else {
            DepKind::Normal
        };

        out.push(DeclaredDep {
            name,
            version_req,
            kind,
        });

        i += 3;
    }
}

fn collect_quoted_strings(s: &str) -> Vec<String> {
    let mut result = Vec::new();
    let mut chars = s.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '"' {
            let mut buf = String::new();
            for inner in chars.by_ref() {
                if inner == '"' {
                    break;
                }
                buf.push(inner);
            }
            result.push(buf);
        }
    }
    result
}

fn extract_sbt_string(stmt: &str) -> Option<String> {
    let start = stmt.find('"')? + 1;
    let end = stmt[start..].find('"')?;
    Some(stmt[start..start + end].to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ManifestParser;

    #[test]
    fn test_parse_build_sbt() {
        let content = r#"
name := "my-project"
version := "0.1.0"
scalaVersion := "2.13.12"

libraryDependencies += "org.typelevel" %% "cats-core" % "2.10.0"
libraryDependencies += "com.typesafe.akka" %% "akka-http" % "10.5.0"
libraryDependencies += "org.scalatest" %% "scalatest" % "3.2.17" % Test
"#;
        let m = SbtParser.parse(content).unwrap();
        assert_eq!(m.ecosystem, "sbt");
        assert_eq!(m.name.as_deref(), Some("my-project"));
        assert_eq!(m.version.as_deref(), Some("0.1.0"));
        assert_eq!(m.dependencies.len(), 3);

        let cats = m
            .dependencies
            .iter()
            .find(|d| d.name.contains("cats-core"))
            .unwrap();
        assert_eq!(cats.version_req.as_deref(), Some("2.10.0"));
        assert_eq!(cats.kind, DepKind::Normal);

        let scalatest = m
            .dependencies
            .iter()
            .find(|d| d.name.contains("scalatest"))
            .unwrap();
        assert_eq!(scalatest.kind, DepKind::Dev);
    }

    #[test]
    fn test_sbt_seq_deps() {
        let content = r#"
libraryDependencies ++= Seq(
  "org.typelevel" %% "cats-core" % "2.10.0",
  "io.circe" %% "circe-core" % "0.14.6"
)
"#;
        let m = SbtParser.parse(content).unwrap();
        assert_eq!(m.dependencies.len(), 2);
    }
}
