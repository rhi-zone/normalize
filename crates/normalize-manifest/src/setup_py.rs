//! Heuristic parser for `setup.py` files (Python/setuptools).
//!
//! Performs regex-free text extraction of `setup()` keyword arguments without
//! executing the file. For eval-backed parsing (which actually runs Python),
//! see `src/eval.rs`.

use crate::pip::parse_pip_requirement;
use crate::{DeclaredDep, DepKind, ManifestError, ManifestParser, ParsedManifest};

/// Heuristic parser for `setup.py` files.
pub struct SetupPyParser;

impl ManifestParser for SetupPyParser {
    fn filename(&self) -> &'static str {
        "setup.py"
    }

    fn parse(&self, content: &str) -> Result<ParsedManifest, ManifestError> {
        let name = extract_str_kwarg(content, "name");
        let version = extract_str_kwarg(content, "version");

        let mut deps = Vec::new();

        // install_requires=[...] — Normal
        if let Some(list) = extract_list(content, "install_requires") {
            for item in parse_string_list(&list) {
                if let Some(mut dep) = parse_pip_requirement(&item) {
                    dep.kind = DepKind::Normal;
                    deps.push(dep);
                }
            }
        }

        // tests_require=[...] — Dev
        if let Some(list) = extract_list(content, "tests_require") {
            for item in parse_string_list(&list) {
                if let Some(mut dep) = parse_pip_requirement(&item) {
                    dep.kind = DepKind::Dev;
                    deps.push(dep);
                }
            }
        }

        // extras_require={...} — Dev for dev/test/testing/tests groups, Optional otherwise
        if let Some(block) = extract_braces(content, "extras_require") {
            deps.extend(parse_extras_require(&block));
        }

        Ok(ParsedManifest {
            ecosystem: "python",
            name,
            version,
            dependencies: deps,
        })
    }
}

// ── Extraction helpers ────────────────────────────────────────────────────────

/// Extract the value of a `key="..."` or `key='...'` keyword argument from
/// the setup() call. Returns the unquoted string.
fn extract_str_kwarg(content: &str, key: &str) -> Option<String> {
    // Match `key=` followed by optional whitespace then a quoted string.
    let search = format!("{key}=");
    let pos = content.find(&search)?;
    let after = content[pos + search.len()..].trim_start();
    extract_quoted_string(after)
}

/// Extract the content of a `[...]` bracket block following `key=[`.
/// Handles multiline lists and nested brackets.
fn extract_list(content: &str, key: &str) -> Option<String> {
    let search = format!("{key}=[");
    let pos = content.find(&search)?;
    let after = &content[pos + search.len()..];
    Some(collect_until_close(after, '[', ']'))
}

/// Extract the content of a `{...}` brace block following `key={`.
fn extract_braces(content: &str, key: &str) -> Option<String> {
    let search = format!("{key}={{");
    let pos = content.find(&search)?;
    let after = &content[pos + search.len()..];
    Some(collect_until_close(after, '{', '}'))
}

/// Collect characters until the matching close delimiter, tracking depth.
/// The opening delimiter has already been consumed; `after` starts after it.
fn collect_until_close(after: &str, open: char, close: char) -> String {
    let mut depth = 1usize;
    let mut result = String::new();
    for ch in after.chars() {
        if ch == open {
            depth += 1;
            result.push(ch);
        } else if ch == close {
            depth -= 1;
            if depth == 0 {
                break;
            }
            result.push(ch);
        } else {
            result.push(ch);
        }
    }
    result
}

/// Extract a quoted string (single or double quotes) from the start of `s`.
fn extract_quoted_string(s: &str) -> Option<String> {
    let quote = s.chars().next()?;
    if quote != '"' && quote != '\'' {
        return None;
    }
    let rest = &s[1..];
    let end = rest.find(quote)?;
    Some(rest[..end].to_string())
}

/// Parse a list body (content between `[` and `]`) and return each quoted
/// string entry.
fn parse_string_list(body: &str) -> Vec<String> {
    let mut result = Vec::new();
    let mut remaining = body;
    while let Some(quote_pos) = remaining.find(['"', '\'']) {
        let quote = remaining.chars().nth(quote_pos).unwrap();
        let after_open = &remaining[quote_pos + 1..];
        // Find closing quote (not preceded by backslash — simple heuristic)
        match after_open.find(quote) {
            Some(end) => {
                result.push(after_open[..end].to_string());
                remaining = &after_open[end + 1..];
            }
            None => break,
        }
    }
    result
}

/// Parse the body of `extras_require={...}`.
///
/// Structure: `"group": ["pkg1", "pkg2"], "other": [...]`
///
/// Groups matching `dev`, `test`, `testing`, `tests` become Dev; others become
/// Optional.
fn parse_extras_require(body: &str) -> Vec<DeclaredDep> {
    let dev_groups = ["dev", "test", "testing", "tests", "develop", "development"];
    let mut deps = Vec::new();
    let mut remaining = body;

    while let Some(quote_pos) = remaining.find(['"', '\'']) {
        let quote = remaining.chars().nth(quote_pos).unwrap();
        let after_open = &remaining[quote_pos + 1..];
        let key_end = match after_open.find(quote) {
            Some(e) => e,
            None => break,
        };
        let group = after_open[..key_end].to_string();
        remaining = &after_open[key_end + 1..];

        // Find the `[` starting the list for this group
        let bracket_pos = match remaining.find('[') {
            Some(p) => p,
            None => break,
        };
        remaining = &remaining[bracket_pos + 1..];
        let list_body = collect_until_close(remaining, '[', ']');
        // Advance past the list
        let consumed = list_body.len() + 1; // +1 for the closing `]`
        if consumed <= remaining.len() {
            remaining = &remaining[consumed..];
        } else {
            remaining = "";
        }

        let kind = if dev_groups.contains(&group.as_str()) {
            DepKind::Dev
        } else {
            DepKind::Optional
        };

        for item in parse_string_list(&list_body) {
            if let Some(mut dep) = parse_pip_requirement(&item) {
                dep.kind = kind;
                deps.push(dep);
            }
        }
    }

    deps
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ManifestParser;

    #[test]
    fn test_basic_install_requires() {
        let content = r#"
from setuptools import setup

setup(
    name="mypackage",
    version="1.0.0",
    install_requires=[
        "requests>=2.28.0",
        "click>=8.0",
    ],
)
"#;
        let m = SetupPyParser.parse(content).unwrap();
        assert_eq!(m.ecosystem, "python");
        assert_eq!(m.name.as_deref(), Some("mypackage"));
        assert_eq!(m.version.as_deref(), Some("1.0.0"));
        assert_eq!(m.dependencies.len(), 2);

        let req = m
            .dependencies
            .iter()
            .find(|d| d.name == "requests")
            .unwrap();
        assert_eq!(req.version_req.as_deref(), Some(">=2.28.0"));
        assert_eq!(req.kind, DepKind::Normal);

        let click = m.dependencies.iter().find(|d| d.name == "click").unwrap();
        assert_eq!(click.version_req.as_deref(), Some(">=8.0"));
        assert_eq!(click.kind, DepKind::Normal);
    }

    #[test]
    fn test_multiline_install_requires() {
        let content = r#"
setup(
    name='mypkg',
    install_requires=[
        'flask>=2.0',
        'sqlalchemy',
        'celery>=5.0,<6',
    ],
)
"#;
        let m = SetupPyParser.parse(content).unwrap();
        assert_eq!(m.dependencies.len(), 3);

        let flask = m.dependencies.iter().find(|d| d.name == "flask").unwrap();
        assert_eq!(flask.version_req.as_deref(), Some(">=2.0"));

        let sa = m
            .dependencies
            .iter()
            .find(|d| d.name == "sqlalchemy")
            .unwrap();
        assert!(sa.version_req.is_none());
    }

    #[test]
    fn test_tests_require() {
        let content = r#"
setup(
    name='mypkg',
    version='0.1.0',
    install_requires=['requests'],
    tests_require=['pytest>=7.0', 'coverage'],
)
"#;
        let m = SetupPyParser.parse(content).unwrap();

        let normal: Vec<_> = m
            .dependencies
            .iter()
            .filter(|d| d.kind == DepKind::Normal)
            .collect();
        assert_eq!(normal.len(), 1);
        assert_eq!(normal[0].name, "requests");

        let dev: Vec<_> = m
            .dependencies
            .iter()
            .filter(|d| d.kind == DepKind::Dev)
            .collect();
        assert_eq!(dev.len(), 2);
        assert!(dev.iter().any(|d| d.name == "pytest"));
        assert!(dev.iter().any(|d| d.name == "coverage"));

        let pytest = dev.iter().find(|d| d.name == "pytest").unwrap();
        assert_eq!(pytest.version_req.as_deref(), Some(">=7.0"));
    }

    #[test]
    fn test_extras_require_dev_and_optional() {
        let content = r#"
setup(
    name='mypkg',
    version='2.0.0',
    extras_require={
        "dev": ["pytest>=7.0", "black"],
        "test": ["pytest", "coverage"],
        "docs": ["sphinx>=5.0", "myst-parser"],
    },
)
"#;
        let m = SetupPyParser.parse(content).unwrap();

        let dev: Vec<_> = m
            .dependencies
            .iter()
            .filter(|d| d.kind == DepKind::Dev)
            .collect();
        // "dev" group: pytest, black; "test" group: pytest, coverage = 4 entries
        assert_eq!(dev.len(), 4);

        let optional: Vec<_> = m
            .dependencies
            .iter()
            .filter(|d| d.kind == DepKind::Optional)
            .collect();
        // "docs" group: sphinx, myst-parser = 2 entries
        assert_eq!(optional.len(), 2);
        assert!(optional.iter().any(|d| d.name == "sphinx"));
    }

    #[test]
    fn test_extras_require_testing_group() {
        let content = r#"
setup(
    extras_require={
        "testing": ["pytest"],
        "tests": ["coverage"],
    },
)
"#;
        let m = SetupPyParser.parse(content).unwrap();
        let dev: Vec<_> = m
            .dependencies
            .iter()
            .filter(|d| d.kind == DepKind::Dev)
            .collect();
        assert_eq!(dev.len(), 2);
    }

    #[test]
    fn test_no_deps() {
        let content = r#"
from setuptools import setup
setup(
    name="simple",
    version="0.0.1",
)
"#;
        let m = SetupPyParser.parse(content).unwrap();
        assert_eq!(m.name.as_deref(), Some("simple"));
        assert!(m.dependencies.is_empty());
    }
}
