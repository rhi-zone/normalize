//! File length analysis - find longest files in codebase

use crate::output::OutputFormatter;
use crate::path_resolve;
use glob::Pattern;
use normalize_rank::ranked::{
    Column, DiffableRankEntry, RankEntry, format_delta, format_ranked_table,
};
use rayon::prelude::*;
use serde::Serialize;
use std::collections::HashMap;
use std::path::Path;

/// Entry for the By Language breakdown table.
struct ByLanguage<'a> {
    language: &'a str,
    lines: usize,
}

impl RankEntry for ByLanguage<'_> {
    fn columns() -> Vec<Column> {
        vec![Column::left("Language"), Column::right("Lines")]
    }

    fn values(&self) -> Vec<String> {
        vec![self.language.to_string(), self.lines.to_string()]
    }
}

/// File length info
#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
pub struct FileLength {
    pub path: String,
    pub lines: usize,
    pub language: String,
    /// Delta vs baseline (set by `--diff`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delta: Option<f64>,
}

impl RankEntry for FileLength {
    fn columns() -> Vec<Column> {
        vec![Column::right("Lines"), Column::left("Path")]
    }

    fn values(&self) -> Vec<String> {
        let lines_str = match self.delta {
            Some(d) => format!("{} ({})", self.lines, format_delta(d, false)),
            None => self.lines.to_string(),
        };
        vec![lines_str, self.path.clone()]
    }
}

impl DiffableRankEntry for FileLength {
    fn diff_key(&self) -> &str {
        &self.path
    }
    fn diff_score(&self) -> f64 {
        self.lines as f64
    }
    fn set_delta(&mut self, delta: Option<f64>) {
        self.delta = delta;
    }
    fn delta(&self) -> Option<f64> {
        self.delta
    }
}

/// File length report
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct FileLengthReport {
    pub files: Vec<FileLength>,
    pub total_lines: usize,
    pub by_language: HashMap<String, usize>,
    /// Set when `--diff` is used.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diff_ref: Option<String>,
}

impl FileLengthReport {
    fn main_title(&self) -> String {
        let prefix = match &self.diff_ref {
            Some(r) => format!("# Longest Files Diff vs {r}"),
            None => "# Longest Files".to_string(),
        };
        format!("{prefix} — {} lines across all files", self.total_lines)
    }
}

impl OutputFormatter for FileLengthReport {
    fn format_text(&self) -> String {
        let mut out = format_ranked_table(&self.main_title(), &self.files, None);

        if !self.by_language.is_empty() {
            let mut langs: Vec<ByLanguage<'_>> = self
                .by_language
                .iter()
                .map(|(lang, &lines)| ByLanguage {
                    language: lang.as_str(),
                    lines,
                })
                .collect();
            langs.sort_by_key(|b| std::cmp::Reverse(b.lines));
            out.push_str("\n\n");
            out.push_str(&format_ranked_table("## By Language", &langs, None));
        }

        out
    }

    fn format_pretty(&self) -> String {
        let mut out =
            crate::output::pretty_ranked_table(&self.main_title(), &self.files, None, |_e| None);

        if !self.by_language.is_empty() {
            let mut langs: Vec<ByLanguage<'_>> = self
                .by_language
                .iter()
                .map(|(lang, &lines)| ByLanguage {
                    language: lang.as_str(),
                    lines,
                })
                .collect();
            langs.sort_by_key(|b| std::cmp::Reverse(b.lines));
            out.push_str("\n\n");
            out.push_str(&crate::output::pretty_ranked_table(
                "## By Language",
                &langs,
                None,
                |_e| None,
            ));
        }

        out
    }
}

/// Analyze file lengths
pub fn analyze_files(root: &Path, limit: usize, exclude: &[String]) -> FileLengthReport {
    let all_files = path_resolve::all_files(root);
    let files: Vec<_> = all_files
        .iter()
        .filter(|f| f.kind == normalize_path_resolve::PathMatchKind::File)
        .collect();

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
                delta: None,
            })
        })
        .collect();

    let total_lines: usize = file_lengths.iter().map(|f| f.lines).sum();

    let mut by_language: HashMap<String, usize> = HashMap::new();
    for f in &file_lengths {
        *by_language.entry(f.language.clone()).or_insert(0) += f.lines;
    }

    let mut sorted = file_lengths;
    normalize_rank::ranked::rank_and_truncate(
        &mut sorted,
        limit,
        |a, b| b.lines.cmp(&a.lines),
        |f| f.lines as f64,
    );

    FileLengthReport {
        files: sorted,
        total_lines,
        by_language,
        diff_ref: None,
    }
}
