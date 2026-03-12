//! File length analysis - find longest files in codebase

use crate::output::OutputFormatter;
use crate::path_resolve;
use glob::Pattern;
use normalize_analyze::ranked::{Column, RankEntry, format_ranked_table};
use rayon::prelude::*;
use serde::Serialize;
use std::collections::HashMap;
use std::path::Path;

/// File length info
#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
pub struct FileLength {
    pub path: String,
    pub lines: usize,
    pub language: String,
}

impl RankEntry for FileLength {
    fn columns() -> Vec<Column> {
        vec![Column::right("Lines"), Column::left("Path")]
    }

    fn values(&self) -> Vec<String> {
        vec![self.lines.to_string(), self.path.clone()]
    }
}

/// File length report
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct FileLengthReport {
    pub files: Vec<FileLength>,
    pub total_lines: usize,
    pub by_language: HashMap<String, usize>,
}

impl OutputFormatter for FileLengthReport {
    fn format_text(&self) -> String {
        let mut out = format_ranked_table(
            &format!(
                "# Longest Files — {} lines across all files",
                self.total_lines
            ),
            &self.files,
            None,
        );

        if !self.by_language.is_empty() {
            out.push_str("\n\n## By Language\n");
            let mut langs: Vec<_> = self.by_language.iter().collect();
            langs.sort_by(|a, b| b.1.cmp(a.1));
            for (lang, count) in langs {
                out.push_str(&format!("{:>6} lines  {}\n", count, lang));
            }
        }

        out
    }
}

/// Analyze file lengths
pub fn analyze_files(root: &Path, limit: usize, exclude: &[String]) -> FileLengthReport {
    let all_files = path_resolve::all_files(root);
    let files: Vec<_> = all_files.iter().filter(|f| f.kind == "file").collect();

    // Compile exclude patterns
    let excludes: Vec<Pattern> = exclude
        .iter()
        .filter_map(|p| Pattern::new(p).ok())
        .collect();

    let file_lengths: Vec<FileLength> = files
        .par_iter()
        .filter_map(|file| {
            // Skip excluded files
            if excludes.iter().any(|pat| pat.matches(&file.path)) {
                return None;
            }

            let path = root.join(&file.path);
            let lang = normalize_languages::support_for_path(&path)?;

            let content = std::fs::read_to_string(&path).ok()?;
            let lines = content.lines().count();

            Some(FileLength {
                path: file.path.clone(),
                lines,
                language: lang.name().to_string(),
            })
        })
        .collect();

    let total_lines: usize = file_lengths.iter().map(|f| f.lines).sum();

    let mut by_language: HashMap<String, usize> = HashMap::new();
    for f in &file_lengths {
        *by_language.entry(f.language.clone()).or_insert(0) += f.lines;
    }

    let mut sorted = file_lengths;
    normalize_analyze::ranked::rank_and_truncate(
        &mut sorted,
        limit,
        |a, b| b.lines.cmp(&a.lines),
        |f| f.lines as f64,
    );

    FileLengthReport {
        files: sorted,
        total_lines,
        by_language,
    }
}
