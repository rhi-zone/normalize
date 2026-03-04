//! Heuristic parser for `Package.swift` files (Swift Package Manager).
//!
//! Extracts `.package(url: "...", from: "1.0.0")` and similar declarations
//! without executing Swift. Package names are derived from the repository URL
//! (last path component, stripped of `.git`).

use crate::{DeclaredDep, DepKind, ManifestError, ManifestParser, ParsedManifest};

/// Heuristic parser for `Package.swift` files.
pub struct SwiftPmParser;

impl ManifestParser for SwiftPmParser {
    fn filename(&self) -> &'static str {
        "Package.swift"
    }

    fn parse(&self, content: &str) -> Result<ParsedManifest, ManifestError> {
        let mut name = None;
        let version = None;
        let mut deps = Vec::new();

        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with("//") {
                continue;
            }

            // name: "MyPackage"  (inside Package(...))
            if name.is_none()
                && let Some(n) = extract_keyword_string(trimmed, "name:")
            {
                name = Some(n);
            }

            // .package(url: "https://github.com/user/repo.git", from: "1.0.0")
            // .package(url: "...", .upToNextMajor(from: "1.0.0"))
            // .package(url: "...", exact: "1.2.3")
            // .package(url: "...", branch: "main")
            // .package(url: "...", revision: "abc")
            if trimmed.contains(".package(")
                && let Some(dep) = parse_swift_package(trimmed)
            {
                deps.push(dep);
            }
        }

        Ok(ParsedManifest {
            ecosystem: "spm",
            name,
            version,
            dependencies: deps,
        })
    }
}

fn extract_keyword_string(line: &str, keyword: &str) -> Option<String> {
    let idx = line.find(keyword)? + keyword.len();
    let rest = line[idx..].trim();
    let inner = rest.strip_prefix('"')?;
    let end = inner.find('"')?;
    Some(inner[..end].to_string())
}

fn parse_swift_package(line: &str) -> Option<DeclaredDep> {
    // Extract URL
    let url_start = line.find("url:")?;
    let rest = &line[url_start + 4..].trim_start();
    if !rest.starts_with('"') {
        return None;
    }
    let url_end = rest[1..].find('"')?;
    let url = &rest[1..1 + url_end];

    // Derive package name from URL: last path segment, strip .git
    let pkg_name = url
        .trim_end_matches('/')
        .rsplit('/')
        .next()?
        .trim_end_matches(".git")
        .to_string();

    if pkg_name.is_empty() {
        return None;
    }

    // Extract version requirement
    // from: "1.0.0"  →  ">= 1.0.0"
    // exact: "1.2.3"  →  "== 1.2.3"
    // upToNextMajor(from: "1.0.0")  →  "^1.0.0"
    // upToNextMinor(from: "1.0.0")  →  "~>1.0.0"
    // branch: "main" / revision: "..."  →  None
    let version_req = extract_swift_version_req(line);

    Some(DeclaredDep {
        name: pkg_name,
        version_req,
        kind: DepKind::Normal,
    })
}

fn extract_swift_version_req(line: &str) -> Option<String> {
    // Check compound forms before bare `from:` to avoid mismatching `.upToNextMajor(from: ...)`
    if line.contains("upToNextMajor(from:")
        && let Some(s) = extract_keyword_string(line, "upToNextMajor(from:")
    {
        return Some(format!("^{}", s));
    }
    if line.contains("upToNextMinor(from:")
        && let Some(s) = extract_keyword_string(line, "upToNextMinor(from:")
    {
        return Some(format!("~>{}", s));
    }
    if let Some(s) = extract_keyword_string(line, "exact:") {
        return Some(format!("== {}", s));
    }
    if let Some(s) = extract_keyword_string(line, "from:") {
        return Some(format!(">= {}", s));
    }
    // Range form: "1.0.0" ..< "5.0.0"  (exclusive upper) or "1.0.0" ... "5.0.0" (inclusive)
    if let Some(lo) = extract_range_version(line, "..<") {
        return Some(lo);
    }
    if let Some(lo) = extract_range_version(line, "...") {
        return Some(lo);
    }
    // branch/revision — no semver constraint
    None
}

fn extract_range_version(line: &str, op: &str) -> Option<String> {
    let idx = line.find(op)?;
    let before = line[..idx].trim_end();
    // Lower bound: last quoted string before operator
    if !before.ends_with('"') {
        return None;
    }
    let inner_end = before.len() - 1;
    let inner_start = before[..inner_end].rfind('"')? + 1;
    let lower = &before[inner_start..inner_end];
    // Upper bound: first quoted string after operator
    let after = line[idx + op.len()..].trim_start();
    let upper_inner = after.strip_prefix('"')?;
    let upper_end = upper_inner.find('"')?;
    let upper = &upper_inner[..upper_end];
    let cmp = if op == "..<" { "<" } else { "<=" };
    Some(format!(">= {lower}, {cmp} {upper}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ManifestParser;

    #[test]
    fn test_parse_package_swift() {
        let content = r#"// swift-tools-version: 5.9
import PackageDescription

let package = Package(
    name: "MySwiftApp",
    dependencies: [
        .package(url: "https://github.com/apple/swift-argument-parser.git", from: "1.2.0"),
        .package(url: "https://github.com/vapor/vapor.git", .upToNextMajor(from: "4.0.0")),
        .package(url: "https://github.com/nicklockwood/SwiftyJSON.git", exact: "5.0.1"),
        .package(url: "https://github.com/some/pkg.git", branch: "main"),
    ]
)
"#;
        let m = SwiftPmParser.parse(content).unwrap();
        assert_eq!(m.ecosystem, "spm");
        assert_eq!(m.name.as_deref(), Some("MySwiftApp"));
        assert_eq!(m.dependencies.len(), 4);

        let argparser = m
            .dependencies
            .iter()
            .find(|d| d.name == "swift-argument-parser")
            .unwrap();
        assert_eq!(argparser.version_req.as_deref(), Some(">= 1.2.0"));

        let vapor = m.dependencies.iter().find(|d| d.name == "vapor").unwrap();
        assert_eq!(vapor.version_req.as_deref(), Some("^4.0.0"));

        let swifty = m
            .dependencies
            .iter()
            .find(|d| d.name == "SwiftyJSON")
            .unwrap();
        assert_eq!(swifty.version_req.as_deref(), Some("== 5.0.1"));

        let branch_dep = m.dependencies.iter().find(|d| d.name == "pkg").unwrap();
        assert!(branch_dep.version_req.is_none());
    }
}
