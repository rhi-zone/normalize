//! Documentation coverage analysis

use crate::filter::Filter;
use crate::output::OutputFormatter;
use normalize_analyze::ranked::{Column, RankEntry, format_ranked_table};
use normalize_languages::is_test_path;
use serde::Serialize;
use std::collections::HashMap;
use std::path::Path;

/// Doc coverage info for a single file
#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
pub struct FileDocCoverage {
    pub file_path: String,
    pub documented: usize,
    pub total: usize,
}

impl FileDocCoverage {
    /// Bayesian-adjusted coverage for sorting.
    /// Uses Beta(1,1) prior (add 1 success and 1 failure).
    pub fn bayesian_coverage(&self) -> f64 {
        (self.documented as f64 + 1.0) / (self.total as f64 + 2.0)
    }

    pub fn coverage_percent(&self) -> f64 {
        if self.total == 0 {
            100.0 // No callables = trivially 100% documented
        } else {
            100.0 * self.documented as f64 / self.total as f64
        }
    }
}

impl RankEntry for FileDocCoverage {
    fn columns() -> Vec<Column> {
        vec![
            Column::right("%"),
            Column::right("Doc'd"),
            Column::right("Total"),
            Column::left("File"),
        ]
    }

    fn values(&self) -> Vec<String> {
        vec![
            format!("{:.0}%", self.coverage_percent()),
            self.documented.to_string(),
            self.total.to_string(),
            self.file_path.clone(),
        ]
    }
}

/// Documentation coverage report
#[derive(Serialize, schemars::JsonSchema)]
pub struct DocCoverageReport {
    pub total_callables: usize,
    pub documented: usize,
    pub coverage_percent: f64,
    pub by_language: HashMap<String, (usize, usize)>, // (documented, total)
    pub worst_files: Vec<FileDocCoverage>,
}

impl OutputFormatter for DocCoverageReport {
    fn format_text(&self) -> String {
        let mut out = String::new();

        // Per-language breakdown
        if !self.by_language.is_empty() {
            out.push_str(&format!(
                "# Documentation Coverage — {:.0}% ({} of {} documented)\n\n",
                self.coverage_percent, self.documented, self.total_callables
            ));
            out.push_str("## By Language\n");
            let mut langs: Vec<_> = self.by_language.iter().collect();
            langs.sort_by(|a, b| {
                let pct_a = if a.1.1 > 0 {
                    a.1.0 as f64 / a.1.1 as f64
                } else {
                    1.0
                };
                let pct_b = if b.1.1 > 0 {
                    b.1.0 as f64 / b.1.1 as f64
                } else {
                    1.0
                };
                // normalize-syntax-allow: rust/unwrap-in-impl - pct values are finite ratios (0.0-1.0)
                pct_a.partial_cmp(&pct_b).unwrap()
            });
            for (lang, (documented, total)) in langs {
                if *total > 0 {
                    let pct = 100.0 * *documented as f64 / *total as f64;
                    out.push_str(&format!(
                        "  {:>3.0}% ({:>3}/{:>4}) {}\n",
                        pct, documented, total, lang
                    ));
                }
            }
            out.push('\n');
        }

        // Worst files via shared table
        out.push_str(&format_ranked_table(
            "## Worst Coverage",
            &self.worst_files,
            None,
        ));

        out
    }
}

/// Analyze documentation coverage
pub async fn analyze_docs(
    root: &Path,
    limit: usize,
    exclude_interface_impls: bool,
    filter: Option<&Filter>,
) -> DocCoverageReport {
    use crate::extract::{IndexedResolver, InterfaceResolver, OnDemandResolver};
    use crate::path_resolve;

    let all_files = path_resolve::all_files(root);
    let files: Vec<_> = all_files
        .iter()
        .filter(|f| f.kind == normalize_path_resolve::PathMatchKind::File)
        .filter(|f| !is_test_path(Path::new(&f.path)))
        .filter(|f| {
            if let Some(flt) = filter {
                flt.matches(Path::new(&f.path))
            } else {
                true
            }
        })
        .collect();

    // Try to load index for cross-file resolution, fall back to on-demand parsing
    let index = crate::index::open(root).await.ok();
    let resolver: Box<dyn InterfaceResolver> = match &index {
        Some(idx) => Box::new(IndexedResolver::new(idx)),
        None => Box::new(OnDemandResolver::new(root)),
    };

    let mut by_language: HashMap<String, (usize, usize)> = HashMap::new();
    let mut file_coverages: Vec<FileDocCoverage> = Vec::new();

    // Process files sequentially
    for file in &files {
        process_file(
            file,
            root,
            exclude_interface_impls,
            &*resolver,
            &mut by_language,
            &mut file_coverages,
        );
    }

    // Sort by Bayesian coverage (worst first)
    file_coverages.sort_by(|a, b| {
        a.bayesian_coverage()
            .partial_cmp(&b.bayesian_coverage())
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let worst_files: Vec<FileDocCoverage> = file_coverages.into_iter().take(limit).collect();

    // Calculate totals
    let total_callables: usize = by_language.values().map(|(_, t)| t).sum();
    let documented: usize = by_language.values().map(|(d, _)| d).sum();
    let coverage_percent = if total_callables > 0 {
        100.0 * documented as f64 / total_callables as f64
    } else {
        0.0
    };

    DocCoverageReport {
        total_callables,
        documented,
        coverage_percent,
        by_language,
        worst_files,
    }
}

fn process_file(
    file: &crate::path_resolve::PathMatch,
    root: &Path,
    exclude_interface_impls: bool,
    resolver: &dyn crate::extract::InterfaceResolver,
    by_language: &mut HashMap<String, (usize, usize)>,
    file_coverages: &mut Vec<FileDocCoverage>,
) {
    use crate::skeleton::SkeletonExtractor;
    use normalize_languages::SymbolKind;

    let path = root.join(&file.path);
    let lang = normalize_languages::support_for_path(&path);

    // normalize-syntax-allow: rust/unwrap-in-impl - short-circuit: only reached when Some
    if lang.is_none() || lang.unwrap().as_symbols().is_none() {
        return;
    }

    // normalize-syntax-allow: rust/unwrap-in-impl - guarded by is_none() check above
    let lang = lang.unwrap();
    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => return,
    };

    let skeleton_extractor = SkeletonExtractor::new();
    let skeleton = skeleton_extractor
        .extract_with_resolver(&path, &content, Some(resolver))
        .filter_tests();

    let mut documented = 0;
    let mut total = 0;

    fn count_docs(
        symbols: &[normalize_languages::Symbol],
        documented: &mut usize,
        total: &mut usize,
        exclude_interface_impls: bool,
    ) {
        for sym in symbols {
            // Skip interface implementations if configured
            if exclude_interface_impls && sym.is_interface_impl {
                continue;
            }
            match sym.kind {
                SymbolKind::Function | SymbolKind::Method => {
                    *total += 1;
                    if sym.docstring.is_some() {
                        *documented += 1;
                    }
                }
                _ => {}
            }
            count_docs(&sym.children, documented, total, exclude_interface_impls);
        }
    }

    count_docs(
        &skeleton.symbols,
        &mut documented,
        &mut total,
        exclude_interface_impls,
    );

    if total > 0 {
        // Update language stats
        let entry = by_language.entry(lang.name().to_string()).or_insert((0, 0));
        entry.0 += documented;
        entry.1 += total;

        // Add file coverage
        file_coverages.push(FileDocCoverage {
            file_path: file.path.clone(),
            documented,
            total,
        });
    }
}
