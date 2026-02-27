//! CLI command implementations - one module per top-level command.

use crate::config::NormalizeConfig;
use crate::filter::Filter;
use aliases::detect_project_languages;
use std::path::Path;

/// Build a `Filter` from `--exclude` / `--only` patterns, printing any warnings.
/// Returns `None` if both slices are empty (no filtering needed).
pub fn build_filter(root: &Path, exclude: &[String], only: &[String]) -> Option<Filter> {
    if exclude.is_empty() && only.is_empty() {
        return None;
    }
    let config = NormalizeConfig::load(root);
    let languages = detect_project_languages(root);
    let lang_refs: Vec<&str> = languages.iter().map(|s| s.as_str()).collect();
    match Filter::new(exclude, only, &config.aliases, &lang_refs) {
        Ok(f) => {
            for warning in f.warnings() {
                eprintln!("warning: {}", warning);
            }
            Some(f)
        }
        Err(e) => {
            eprintln!("error: {}", e);
            None
        }
    }
}

pub mod aliases;
pub mod analyze;
pub mod context;
pub mod daemon;
pub mod edit;
pub mod facts;
pub mod generate;
pub mod grammars;
pub mod history;
pub mod init;
pub mod package;
pub mod rules;
pub mod sessions;
pub mod text_search;
pub mod tools;
pub mod translate;
pub mod update;
pub mod view;
