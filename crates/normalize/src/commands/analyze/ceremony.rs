//! Ceremony ratio analysis: fraction of callable code that is trait/interface boilerplate.

use crate::output::OutputFormatter;
use rayon::prelude::*;
use serde::Serialize;
use std::collections::HashMap;
use std::path::Path;

/// Ceremony stats for a single language
#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
pub struct CeremonyLangStats {
    /// Total callable symbols (functions + methods)
    pub total: usize,
    /// Count that are interface/trait implementations
    pub interface_impl: usize,
}

impl CeremonyLangStats {
    pub fn ceremony_ratio(&self) -> f64 {
        if self.total == 0 {
            0.0
        } else {
            self.interface_impl as f64 / self.total as f64
        }
    }
}

/// Per-file ceremony breakdown
#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
pub struct FileCeremony {
    pub file_path: String,
    pub total: usize,
    pub interface_impl: usize,
    pub free_functions: usize,
    pub inherent_methods: usize,
    pub ceremony_ratio: f64,
}

/// Report returned by analyze ceremony
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct CeremonyReport {
    /// Total callable symbols across all files
    pub total_functions: usize,
    /// Methods that implement an interface or trait
    pub interface_impl_methods: usize,
    /// Free functions (not part of any impl/class)
    pub free_functions: usize,
    /// Inherent/class methods (not fulfilling an interface contract)
    pub inherent_methods: usize,
    /// Fraction of callables that are interface implementation boilerplate
    pub ceremony_ratio: f64,
    /// Per-language breakdown
    pub by_language: HashMap<String, CeremonyLangStats>,
    /// Files with the highest ceremony ratio (at least 2 callables)
    pub top_files: Vec<FileCeremony>,
}

impl OutputFormatter for CeremonyReport {
    fn format_text(&self) -> String {
        let mut lines = Vec::new();

        lines.push("# Ceremony Ratio".to_string());
        lines.push(String::new());
        lines.push(format!(
            "Overall: {:.1}% ceremony ({} of {} callables are interface boilerplate)",
            self.ceremony_ratio * 100.0,
            self.interface_impl_methods,
            self.total_functions,
        ));
        lines.push(format!(
            "  Interface impl methods : {:>5}  ({:.1}%)",
            self.interface_impl_methods,
            self.ceremony_ratio * 100.0
        ));
        lines.push(format!(
            "  Inherent/class methods : {:>5}  ({:.1}%)",
            self.inherent_methods,
            if self.total_functions > 0 {
                self.inherent_methods as f64 / self.total_functions as f64 * 100.0
            } else {
                0.0
            }
        ));
        lines.push(format!(
            "  Free functions         : {:>5}  ({:.1}%)",
            self.free_functions,
            if self.total_functions > 0 {
                self.free_functions as f64 / self.total_functions as f64 * 100.0
            } else {
                0.0
            }
        ));

        if !self.by_language.is_empty() {
            lines.push(String::new());
            lines.push("## By Language".to_string());
            let mut langs: Vec<_> = self.by_language.iter().collect();
            langs.sort_by(|a, b| {
                b.1.ceremony_ratio()
                    .partial_cmp(&a.1.ceremony_ratio())
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            for (lang, stats) in langs {
                if stats.total > 0 {
                    lines.push(format!(
                        "  {:>5.1}%  ({:>4}/{:>4})  {}",
                        stats.ceremony_ratio() * 100.0,
                        stats.interface_impl,
                        stats.total,
                        lang,
                    ));
                }
            }
        }

        if !self.top_files.is_empty() {
            lines.push(String::new());
            lines.push("## Highest-Ceremony Files".to_string());
            for f in &self.top_files {
                lines.push(format!(
                    "  {:>5.1}%  ({:>3}/{:>3})  {}",
                    f.ceremony_ratio * 100.0,
                    f.interface_impl,
                    f.total,
                    f.file_path,
                ));
            }
        }

        lines.join("\n")
    }
}

/// Analyze ceremony ratio across a codebase
pub fn analyze_ceremony(root: &Path, limit: usize) -> CeremonyReport {
    use crate::path_resolve;
    use normalize_languages::SymbolKind;

    let all_files = path_resolve::all_files(root);

    // Per-file ceremony extraction (parallel)
    let per_file: Vec<(String, String, usize, usize, usize, usize)> = all_files
        .par_iter()
        .filter(|f| f.kind == "file")
        .filter_map(|file| {
            let path = root.join(&file.path);
            let lang = match normalize_languages::support_for_path(&path) {
                Some(l) if l.has_symbols() => l,
                _ => return None,
            };

            let content = std::fs::read_to_string(&path).ok()?;

            let skeleton_extractor = crate::skeleton::SkeletonExtractor::new();
            let skeleton = skeleton_extractor
                .extract_with_resolver(&path, &content, None)
                .filter_tests();

            let mut file_total = 0usize;
            let mut file_interface_impl = 0usize;
            let mut file_free = 0usize;
            let mut file_inherent = 0usize;

            fn walk(
                symbols: &[normalize_languages::Symbol],
                file_total: &mut usize,
                file_interface_impl: &mut usize,
                file_free: &mut usize,
                file_inherent: &mut usize,
            ) {
                for sym in symbols {
                    match sym.kind {
                        SymbolKind::Function => {
                            *file_total += 1;
                            if sym.is_interface_impl {
                                *file_interface_impl += 1;
                            } else {
                                *file_free += 1;
                            }
                        }
                        SymbolKind::Method => {
                            *file_total += 1;
                            if sym.is_interface_impl {
                                *file_interface_impl += 1;
                            } else {
                                *file_inherent += 1;
                            }
                        }
                        _ => {}
                    }
                    walk(
                        &sym.children,
                        file_total,
                        file_interface_impl,
                        file_free,
                        file_inherent,
                    );
                }
            }

            walk(
                &skeleton.symbols,
                &mut file_total,
                &mut file_interface_impl,
                &mut file_free,
                &mut file_inherent,
            );

            if file_total == 0 {
                return None;
            }

            Some((
                file.path.clone(),
                lang.name().to_string(),
                file_total,
                file_interface_impl,
                file_free,
                file_inherent,
            ))
        })
        .collect();

    // Aggregate results
    let mut by_language: HashMap<String, CeremonyLangStats> = HashMap::new();
    let mut file_ceremonies: Vec<FileCeremony> = Vec::new();
    let mut total_functions = 0usize;
    let mut interface_impl_methods = 0usize;
    let mut free_functions = 0usize;
    let mut inherent_methods = 0usize;

    for (file_path, lang_name, file_total, file_interface_impl, file_free, file_inherent) in
        per_file
    {
        total_functions += file_total;
        interface_impl_methods += file_interface_impl;
        free_functions += file_free;
        inherent_methods += file_inherent;

        let entry = by_language.entry(lang_name).or_insert(CeremonyLangStats {
            total: 0,
            interface_impl: 0,
        });
        entry.total += file_total;
        entry.interface_impl += file_interface_impl;

        let ratio = file_interface_impl as f64 / file_total as f64;
        file_ceremonies.push(FileCeremony {
            file_path,
            total: file_total,
            interface_impl: file_interface_impl,
            free_functions: file_free,
            inherent_methods: file_inherent,
            ceremony_ratio: ratio,
        });
    }

    // Sort by ceremony ratio descending, then by total descending for tie-breaking
    file_ceremonies.sort_by(|a, b| {
        b.ceremony_ratio
            .partial_cmp(&a.ceremony_ratio)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(b.total.cmp(&a.total))
    });

    // Only keep files with at least 2 callables (1-callable files are noise)
    let top_files = file_ceremonies
        .into_iter()
        .filter(|f| f.total >= 2)
        .take(limit)
        .collect();

    let ceremony_ratio = if total_functions > 0 {
        interface_impl_methods as f64 / total_functions as f64
    } else {
        0.0
    };

    CeremonyReport {
        total_functions,
        interface_impl_methods,
        free_functions,
        inherent_methods,
        ceremony_ratio,
        by_language,
        top_files,
    }
}
