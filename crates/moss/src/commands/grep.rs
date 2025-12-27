//! Grep command - search file contents for a pattern.

use crate::commands::filter::detect_project_languages;
use crate::config::MossConfig;
use crate::filter::Filter;
use crate::grep;
use crate::output::{OutputFormat, OutputFormatter};
use std::path::Path;

/// Search file contents for a pattern
pub fn cmd_grep(
    pattern: &str,
    root: Option<&Path>,
    limit: usize,
    ignore_case: bool,
    json: bool,
    jq: Option<&str>,
    exclude: &[String],
    only: &[String],
) -> i32 {
    let root = root
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| std::env::current_dir().unwrap());

    // Build filter for --exclude and --only
    let filter = if !exclude.is_empty() || !only.is_empty() {
        let config = MossConfig::load(&root);
        let languages = detect_project_languages(&root);
        let lang_refs: Vec<&str> = languages.iter().map(|s| s.as_str()).collect();

        match Filter::new(exclude, only, &config.filter, &lang_refs) {
            Ok(f) => {
                for warning in f.warnings() {
                    eprintln!("warning: {}", warning);
                }
                Some(f)
            }
            Err(e) => {
                eprintln!("error: {}", e);
                return 1;
            }
        }
    } else {
        None
    };

    match grep::grep(pattern, &root, filter.as_ref(), limit, ignore_case) {
        Ok(result) => {
            let format = OutputFormat::from_flags(json, jq);
            if result.matches.is_empty() && !format.is_json() {
                eprintln!("No matches found for: {}", pattern);
                return 1;
            }
            result.print(&format);
            0
        }
        Err(e) => {
            eprintln!("Error: {}", e);
            1
        }
    }
}
