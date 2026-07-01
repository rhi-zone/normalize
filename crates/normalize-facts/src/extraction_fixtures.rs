//! Golden-diff harness for language extraction fixtures.
//!
//! Fixture cases live under `<root>/<lang>/<case>/` as an `input.<ext>` +
//! `expected.json` pair. The harness parses the input with [`SymbolParser`]
//! (symbols, imports, calls) and diffs the actual extraction against the
//! expected JSON. This is the engine behind `normalize structure test-fixtures`
//! and the `normalize-languages` fixture integration test — it owns
//! `SymbolParser`, so the diff engine + fixture discovery live here rather than
//! embedded in the CLI service layer.

use std::path::{Path, PathBuf};

use crate::SymbolParser;

// ---------------------------------------------------------------------------
// Case discovery
// ---------------------------------------------------------------------------

/// One discovered fixture case.
pub struct FixtureCase {
    /// Human-readable name like `"rust/basic-function"`.
    pub name: String,
    /// The input source file (`input.<ext>`).
    pub input: PathBuf,
    /// The `expected.json` file (may not exist yet in `--update` mode).
    pub expected_json: PathBuf,
}

/// Outcome of running a single fixture case.
pub struct FixtureCaseResult {
    /// The case name (mirrors [`FixtureCase::name`]).
    pub case: String,
    /// Whether the actual extraction matched `expected.json`.
    pub passed: bool,
    /// Human-readable diff lines (empty when passed).
    pub diff: Vec<String>,
}

/// Discover fixture cases under `root`, optionally filtered to one language dir.
///
/// Walks `<root>/<lang>/<case>/`, treating any case directory that contains an
/// `input.*` file as a fixture (whether or not `expected.json` exists yet).
pub fn discover_cases(root: &Path, lang_filter: Option<&str>) -> std::io::Result<Vec<FixtureCase>> {
    let mut cases: Vec<FixtureCase> = Vec::new();

    let mut lang_entries: Vec<_> = std::fs::read_dir(root)?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    lang_entries.sort();

    for lang_dir in &lang_entries {
        let lang_name = match lang_dir.file_name().and_then(|n| n.to_str()) {
            Some(n) => n.to_string(),
            None => continue,
        };

        if let Some(filter) = lang_filter
            && lang_name != filter
        {
            continue;
        }

        let mut case_entries: Vec<_> = std::fs::read_dir(lang_dir)?
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.is_dir())
            .collect();
        case_entries.sort();

        for case_dir in &case_entries {
            let case_name = match case_dir.file_name().and_then(|n| n.to_str()) {
                Some(n) => n.to_string(),
                None => continue,
            };

            // Find input.<ext> in the case directory.
            let input = match find_input_file(case_dir) {
                Some(p) => p,
                None => continue, // no input file → not an extraction fixture case
            };

            let expected_json = case_dir.join("expected.json");
            cases.push(FixtureCase {
                name: format!("{lang_name}/{case_name}"),
                input,
                expected_json,
            });
        }
    }

    Ok(cases)
}

fn find_input_file(dir: &Path) -> Option<PathBuf> {
    let entries = std::fs::read_dir(dir).ok()?;
    for entry in entries.filter_map(|e| e.ok()) {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        if let Some(stem) = path.file_stem().and_then(|s| s.to_str())
            && stem == "input"
        {
            return Some(path);
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Expected-schema structs (expected.json)
// ---------------------------------------------------------------------------

/// The schema for `expected.json` in an extraction fixture case.
#[derive(serde::Serialize, serde::Deserialize, Default)]
struct ExtractionExpected {
    #[serde(default)]
    exhaustive: bool,
    #[serde(default)]
    symbols: Vec<ExpectedSymbol>,
    #[serde(default)]
    imports: Vec<ExpectedImport>,
    #[serde(default)]
    calls: Vec<ExpectedCall>,
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
struct ExpectedSymbol {
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    line: Option<usize>,
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
struct ExpectedImport {
    #[serde(skip_serializing_if = "Option::is_none")]
    module: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    line: Option<usize>,
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
struct ExpectedCall {
    callee: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    line: Option<usize>,
}

// ---------------------------------------------------------------------------
// Case runner
// ---------------------------------------------------------------------------

/// Run one fixture case.
///
/// With `update = true`, overwrites `expected.json` with the actual extraction
/// (bootstrap mode). Otherwise diffs actual vs. expected and reports mismatches.
pub fn run_case(case: &FixtureCase, update: bool) -> FixtureCaseResult {
    let content = match std::fs::read_to_string(&case.input) {
        Ok(s) => s,
        Err(e) => {
            return FixtureCaseResult {
                case: case.name.clone(),
                passed: false,
                diff: vec![format!("Failed to read input file: {e}")],
            };
        }
    };

    let mut parser = SymbolParser::new();

    // Extract symbols.
    let actual_symbols = parser.parse_file(&case.input, &content).unwrap_or_default();

    // Extract imports.
    let actual_imports = parser.parse_imports(&case.input, &content);

    // Extract calls: iterate over all top-level symbols and collect their callees.
    let mut actual_calls: Vec<(String, usize)> = Vec::new();
    for sym in &actual_symbols {
        let callees = parser.find_callees_for_symbol(&case.input, &content, sym);
        for (callee, line, _, _) in callees {
            actual_calls.push((callee, line));
        }
    }
    actual_calls.sort();
    actual_calls.dedup();

    if update {
        // Build an ExtractionExpected from actual data.
        let expected = ExtractionExpected {
            exhaustive: false,
            symbols: actual_symbols
                .iter()
                .map(|s| ExpectedSymbol {
                    name: s.name.clone(),
                    kind: Some(s.kind.as_str().to_string()),
                    line: Some(s.start_line),
                })
                .collect(),
            imports: actual_imports
                .iter()
                .map(|i| ExpectedImport {
                    module: i.module.clone(),
                    name: Some(i.name.clone()),
                    line: Some(i.line),
                })
                .collect(),
            calls: actual_calls
                .iter()
                .map(|(callee, line)| ExpectedCall {
                    callee: callee.clone(),
                    line: Some(*line),
                })
                .collect(),
        };
        let json = serde_json::to_string_pretty(&expected).unwrap_or_else(|_| "{}".to_string());
        if let Err(e) = std::fs::write(&case.expected_json, json) {
            return FixtureCaseResult {
                case: case.name.clone(),
                passed: false,
                diff: vec![format!("Failed to write expected.json: {e}")],
            };
        }
        return FixtureCaseResult {
            case: case.name.clone(),
            passed: true,
            diff: Vec::new(),
        };
    }

    // Load expected.json.
    let expected: ExtractionExpected = if case.expected_json.exists() {
        match std::fs::read_to_string(&case.expected_json)
            .map_err(|e| e.to_string())
            .and_then(|s| serde_json::from_str(&s).map_err(|e| e.to_string()))
        {
            Ok(v) => v,
            Err(e) => {
                return FixtureCaseResult {
                    case: case.name.clone(),
                    passed: false,
                    diff: vec![format!("Failed to read expected.json: {e}")],
                };
            }
        }
    } else {
        return FixtureCaseResult {
            case: case.name.clone(),
            passed: false,
            diff: vec!["expected.json missing — run with --update to create it".to_string()],
        };
    };

    let mut diff: Vec<String> = Vec::new();

    // Check symbols.
    for exp in &expected.symbols {
        let found = actual_symbols.iter().any(|s| {
            s.name == exp.name
                && exp.kind.as_deref().is_none_or(|k| s.kind.as_str() == k)
                && exp.line.is_none_or(|l| s.start_line == l)
        });
        if !found {
            let desc = format!(
                "{}{}{}",
                exp.name,
                exp.kind
                    .as_deref()
                    .map_or(String::new(), |k| format!(" kind={k}")),
                exp.line.map_or(String::new(), |l| format!(" line={l}")),
            );
            diff.push(format!("missing symbol: {desc}"));
        }
    }

    if expected.exhaustive {
        for sym in &actual_symbols {
            let expected_any = expected.symbols.iter().any(|e| e.name == sym.name);
            if !expected_any {
                diff.push(format!(
                    "unexpected symbol: {} (kind={}) line={}",
                    sym.name,
                    sym.kind.as_str(),
                    sym.start_line
                ));
            }
        }
    }

    // Check imports.
    for exp in &expected.imports {
        let found = actual_imports.iter().any(|i| {
            exp.name.as_deref().is_none_or(|n| i.name == n)
                && exp
                    .module
                    .as_deref()
                    .is_none_or(|m| i.module.as_deref() == Some(m))
                && exp.line.is_none_or(|l| i.line == l)
        });
        if !found {
            let desc = format!(
                "{}{}{}",
                exp.name.as_deref().unwrap_or("*"),
                exp.module
                    .as_deref()
                    .map_or(String::new(), |m| format!(" from={m}")),
                exp.line.map_or(String::new(), |l| format!(" line={l}")),
            );
            diff.push(format!("missing import: {desc}"));
        }
    }

    if expected.exhaustive {
        for imp in &actual_imports {
            let expected_any = expected
                .imports
                .iter()
                .any(|e| e.name.as_deref().is_none_or(|n| n == imp.name));
            if !expected_any {
                diff.push(format!(
                    "unexpected import: {} from={} line={}",
                    imp.name,
                    imp.module.as_deref().unwrap_or("(none)"),
                    imp.line
                ));
            }
        }
    }

    // Check calls.
    for exp in &expected.calls {
        let found = actual_calls
            .iter()
            .any(|(callee, line)| *callee == exp.callee && exp.line.is_none_or(|l| *line == l));
        if !found {
            let desc = format!(
                "{}{}",
                exp.callee,
                exp.line.map_or(String::new(), |l| format!(" line={l}")),
            );
            diff.push(format!("missing call: {desc}"));
        }
    }

    if expected.exhaustive {
        for (callee, line) in &actual_calls {
            let expected_any = expected.calls.iter().any(|e| e.callee == *callee);
            if !expected_any {
                diff.push(format!("unexpected call: {callee} line={line}"));
            }
        }
    }

    diff.sort();

    FixtureCaseResult {
        case: case.name.clone(),
        passed: diff.is_empty(),
        diff,
    }
}
