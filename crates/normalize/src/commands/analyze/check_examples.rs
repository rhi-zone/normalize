//! Validate example references in documentation

use crate::output::OutputFormatter;
use serde::Serialize;
use std::path::Path;

/// A missing example reference
#[derive(Debug, Serialize, schemars::JsonSchema)]
struct MissingExample {
    doc_file: String,
    line: usize,
    reference: String, // path#name
}

/// Example references check report
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct CheckExamplesReport {
    defined_examples: usize,
    references_found: usize,
    missing: Vec<MissingExample>,
}

impl OutputFormatter for CheckExamplesReport {
    fn format_text(&self) -> String {
        let mut lines = Vec::new();
        lines.push("Example Reference Check".to_string());
        lines.push(String::new());
        lines.push(format!("Defined examples: {}", self.defined_examples));
        lines.push(format!("References found: {}", self.references_found));
        lines.push(String::new());

        if self.missing.is_empty() {
            lines.push("All example references are valid.".to_string());
        } else {
            lines.push(format!("Missing examples ({}):", self.missing.len()));
            lines.push(String::new());
            for m in &self.missing {
                lines.push(format!(
                    "  {}:{}: {{{{{}}}}}",
                    m.doc_file, m.line, m.reference
                ));
            }
        }

        lines.join("\n")
    }
}

/// Check that all example references have matching markers
pub fn cmd_check_examples(root: &Path, json: bool) -> i32 {
    use regex::Regex;
    use std::collections::HashSet;

    // Find all example markers in source files: // [example: name] ... // [/example]
    let marker_start_re = Regex::new(r"//\s*\[example:\s*([^\]]+)\]").unwrap();

    // Find all example references in docs: {{example: path#name}}
    let ref_re = Regex::new(r"\{\{example:\s*([^}]+)\}\}").unwrap();

    // Collect all defined examples: (file, name)
    let mut defined_examples: HashSet<String> = HashSet::new();

    // Walk source files
    for entry in walkdir::WalkDir::new(root)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| {
            let path = e.path();
            path.is_file()
                && !path
                    .components()
                    .any(|c| c.as_os_str().to_string_lossy().starts_with('.'))
        })
    {
        let path = entry.path();
        let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("");

        // Only check source files (where we'd have // [example:] markers)
        if !matches!(
            ext,
            "rs" | "py" | "js" | "ts" | "tsx" | "jsx" | "go" | "java" | "c" | "cpp" | "rb"
        ) {
            continue;
        }

        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let rel_path = path
            .strip_prefix(root)
            .unwrap_or(path)
            .display()
            .to_string();

        for cap in marker_start_re.captures_iter(&content) {
            let name = cap[1].trim();
            // Key: path#name
            let key = format!("{}#{}", rel_path, name);
            defined_examples.insert(key);
        }
    }

    // Find all references in markdown files
    let mut missing: Vec<MissingExample> = Vec::new();
    let mut refs_found = 0;

    for entry in walkdir::WalkDir::new(root)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path().extension().and_then(|s| s.to_str()) == Some("md")
                && !e
                    .path()
                    .components()
                    .any(|c| c.as_os_str().to_string_lossy().starts_with('.'))
        })
    {
        let path = entry.path();
        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let rel_path = path
            .strip_prefix(root)
            .unwrap_or(path)
            .display()
            .to_string();

        let mut in_code_block = false;
        for (line_num, line) in content.lines().enumerate() {
            // Track fenced code blocks
            if line.trim().starts_with("```") {
                in_code_block = !in_code_block;
                continue;
            }
            if in_code_block {
                continue;
            }

            for cap in ref_re.captures_iter(line) {
                // Skip if match is inside backticks (inline code)
                let match_start = cap.get(0).unwrap().start();
                let match_end = cap.get(0).unwrap().end();
                let before = &line[..match_start];
                let after = &line[match_end..];

                // Count backticks before match - odd count means we're inside inline code
                if before.chars().filter(|&c| c == '`').count() % 2 == 1 && after.contains('`') {
                    continue;
                }

                refs_found += 1;
                let reference = cap[1].trim();

                // Reference should be path#name
                if !defined_examples.contains(reference) {
                    missing.push(MissingExample {
                        doc_file: rel_path.clone(),
                        line: line_num + 1,
                        reference: reference.to_string(),
                    });
                }
            }
        }
    }

    let report = CheckExamplesReport {
        defined_examples: defined_examples.len(),
        references_found: refs_found,
        missing,
    };
    let config = crate::config::NormalizeConfig::load(root);
    let format =
        crate::output::OutputFormat::from_cli(json, false, None, false, false, &config.pretty);
    report.print(&format);

    if report.missing.is_empty() { 0 } else { 1 }
}
