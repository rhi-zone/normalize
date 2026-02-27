//! Symbol search functionality for view command.

use crate::{index, skeleton};
use normalize_languages::support_for_path;
use std::path::Path;

/// Parsed symbol query with optional file and parent hints.
struct SymbolQuery {
    file_hint: Option<String>,
    parent_hint: Option<String>,
    symbol_name: String,
}

/// Check if a file has language support (symbols can be extracted)
pub fn has_language_support(path: &str) -> bool {
    support_for_path(Path::new(path))
        .map(|lang| lang.has_symbols())
        .unwrap_or(false)
}

/// Search for symbols in the index by name.
/// Supports qualified names like "ClassName/method" or "file.rs/ClassName/method"
pub fn search_symbols(query: &str, root: &Path) -> Vec<index::SymbolMatch> {
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(search_symbols_async(query, root))
}

async fn search_symbols_async(query: &str, root: &Path) -> Vec<index::SymbolMatch> {
    let parsed = parse_symbol_query(query);

    // Try index first - if enabled, use it (or build it if empty)
    if let Some(mut idx) = index::open_if_enabled(root).await {
        let stats = idx.call_graph_stats().await.unwrap_or_default();
        if stats.symbols == 0 {
            eprintln!("Building symbol index...");
            if let Err(e) = idx.refresh_call_graph().await {
                eprintln!("Warning: failed to build index: {}", e);
                return search_symbols_unindexed(query, root);
            }
        }
        if let Ok(mut symbols) = idx.find_symbols(&parsed.symbol_name, None, true, 50).await {
            // Filter by parent hint if provided
            if let Some(ref parent) = parsed.parent_hint {
                let parent_lower = parent.to_lowercase();
                symbols.retain(|s| {
                    s.parent
                        .as_ref()
                        .map(|p| p.to_lowercase().contains(&parent_lower))
                        .unwrap_or(false)
                });
            }
            // Filter by file hint if provided
            if let Some(ref file) = parsed.file_hint {
                let file_lower = file.to_lowercase();
                symbols.retain(|s| s.file.to_lowercase().contains(&file_lower));
            }
            if !symbols.is_empty() {
                symbols.truncate(10);
                return symbols;
            }
        }
    }

    // Fallback: walk filesystem and parse files (only if index disabled)
    search_symbols_unindexed(query, root)
}

/// Parse a symbol query like "Tsx/format_import" or "typescript.rs/Tsx/format_import"
fn parse_symbol_query(query: &str) -> SymbolQuery {
    let parts: Vec<&str> = query.split('/').collect();
    match parts.len() {
        1 => SymbolQuery {
            file_hint: None,
            parent_hint: None,
            symbol_name: parts[0].to_string(),
        },
        2 => {
            // Could be "Parent/method" or "file.rs/symbol"
            if parts[0].contains('.') && !parts[0].starts_with('.') {
                SymbolQuery {
                    file_hint: Some(parts[0].to_string()),
                    parent_hint: None,
                    symbol_name: parts[1].to_string(),
                }
            } else {
                SymbolQuery {
                    file_hint: None,
                    parent_hint: Some(parts[0].to_string()),
                    symbol_name: parts[1].to_string(),
                }
            }
        }
        _ => {
            let symbol_name = parts.last().unwrap().to_string();
            let parent_hint = parts.get(parts.len() - 2).map(|s| s.to_string());
            let file_hint = if parts.len() > 2 {
                Some(parts[..parts.len() - 2].join("/"))
            } else {
                None
            };
            SymbolQuery {
                file_hint,
                parent_hint,
                symbol_name,
            }
        }
    }
}

/// Search for symbols by walking filesystem and parsing files
fn search_symbols_unindexed(query: &str, root: &Path) -> Vec<index::SymbolMatch> {
    use ignore::WalkBuilder;
    use nucleo_matcher::{Config, Matcher};

    let query_lower = query.to_lowercase();
    let mut matcher = Matcher::new(Config::DEFAULT);
    let mut matches = Vec::new();

    let walker = WalkBuilder::new(root).hidden(true).git_ignore(true).build();
    let extractor = skeleton::SkeletonExtractor::new();

    for entry in walker.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }

        let Some(lang) = support_for_path(path) else {
            continue;
        };
        if !lang.has_symbols() {
            continue;
        }

        let Ok(content) = std::fs::read_to_string(path) else {
            continue;
        };

        let result = extractor.extract(path, &content);
        let rel_path = path.strip_prefix(root).unwrap_or(path);

        collect_matching_symbols(
            &result.symbols,
            &query_lower,
            &mut matcher,
            &rel_path.to_string_lossy(),
            None,
            &mut matches,
        );

        if matches.len() >= 20 {
            break;
        }
    }

    matches.sort_by(|a, b| b.1.cmp(&a.1));
    matches.into_iter().take(10).map(|(m, _)| m).collect()
}

/// Compute trigrams (3-char sliding windows) of a string.
fn trigrams(s: &str) -> std::collections::HashSet<[char; 3]> {
    let chars: Vec<char> = s.chars().collect();
    chars.windows(3).map(|w| [w[0], w[1], w[2]]).collect()
}

/// Asymmetric trigram containment: |trigrams(query) ∩ trigrams(candidate)| / |trigrams(query)|
/// Measures how much of the query appears in the candidate — good for prefix/substring/light-typo matches.
fn trigram_containment(
    query_trigrams: &std::collections::HashSet<[char; 3]>,
    candidate: &str,
) -> f32 {
    if query_trigrams.is_empty() {
        return 0.0;
    }
    let c_trigrams = trigrams(candidate);
    let intersection = query_trigrams.intersection(&c_trigrams).count();
    intersection as f32 / query_trigrams.len() as f32
}

/// Suggest symbols by trigram containment when exact/fuzzy search finds nothing.
/// Skips if query has fewer than 4 characters (too short to produce meaningful trigrams).
/// Returns up to `limit` symbols with containment score ≥ `threshold`, sorted by score descending.
pub fn suggest_symbols_trigram(
    query: &str,
    root: &Path,
    threshold: f32,
    limit: usize,
) -> Vec<(index::SymbolMatch, f32)> {
    use ignore::WalkBuilder;

    let query_lower = query.to_lowercase();
    if query_lower.chars().count() < 4 {
        return Vec::new();
    }
    let q_trigrams = trigrams(&query_lower);
    if q_trigrams.is_empty() {
        return Vec::new();
    }

    let mut scored: Vec<(index::SymbolMatch, f32)> = Vec::new();
    let walker = WalkBuilder::new(root).hidden(true).git_ignore(true).build();
    let extractor = skeleton::SkeletonExtractor::new();

    for entry in walker.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let Some(lang) = normalize_languages::support_for_path(path) else {
            continue;
        };
        if !lang.has_symbols() {
            continue;
        }
        let Ok(content) = std::fs::read_to_string(path) else {
            continue;
        };
        let result = extractor.extract(path, &content);
        let rel_path = path.strip_prefix(root).unwrap_or(path);
        let file_str = rel_path.to_string_lossy().to_string();

        collect_trigram_symbols(
            &result.symbols,
            &q_trigrams,
            threshold,
            &file_str,
            None,
            &mut scored,
        );
    }
    scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    scored.truncate(limit);
    scored
}

fn collect_trigram_symbols(
    symbols: &[skeleton::SkeletonSymbol],
    q_trigrams: &std::collections::HashSet<[char; 3]>,
    threshold: f32,
    file: &str,
    parent: Option<&str>,
    scored: &mut Vec<(index::SymbolMatch, f32)>,
) {
    for sym in symbols {
        let name_lower = sym.name.to_lowercase();
        let score = trigram_containment(q_trigrams, &name_lower);
        if score >= threshold {
            scored.push((
                index::SymbolMatch {
                    name: sym.name.clone(),
                    kind: sym.kind.as_str().to_string(),
                    file: file.to_string(),
                    start_line: sym.start_line,
                    end_line: sym.end_line,
                    parent: parent.map(|s| s.to_string()),
                },
                score,
            ));
        }
        collect_trigram_symbols(
            &sym.children,
            q_trigrams,
            threshold,
            file,
            Some(&sym.name),
            scored,
        );
    }
}

fn collect_matching_symbols(
    symbols: &[skeleton::SkeletonSymbol],
    query: &str,
    matcher: &mut nucleo_matcher::Matcher,
    file: &str,
    parent: Option<&str>,
    matches: &mut Vec<(index::SymbolMatch, u32)>,
) {
    use nucleo_matcher::Utf32Str;
    use nucleo_matcher::pattern::{AtomKind, CaseMatching, Normalization, Pattern};

    let pattern = Pattern::new(
        query,
        CaseMatching::Ignore,
        Normalization::Smart,
        AtomKind::Fuzzy,
    );

    for sym in symbols {
        let name_lower = sym.name.to_lowercase();
        let mut buf = Vec::new();
        let haystack = Utf32Str::new(&name_lower, &mut buf);

        if let Some(score) = pattern.score(haystack, matcher) {
            matches.push((
                index::SymbolMatch {
                    name: sym.name.clone(),
                    kind: sym.kind.as_str().to_string(),
                    file: file.to_string(),
                    start_line: sym.start_line,
                    end_line: sym.end_line,
                    parent: parent.map(|s| s.to_string()),
                },
                score,
            ));
        }

        collect_matching_symbols(
            &sym.children,
            query,
            matcher,
            file,
            Some(&sym.name),
            matches,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trigram_containment() {
        let q = "xmd_view_smbl";
        let q_tri = trigrams(&q.to_lowercase());
        let score = trigram_containment(&q_tri, "cmd_view_symbol_direct");
        assert!(score >= 0.5, "expected >= 0.5, got {}", score);
    }

    #[test]
    fn test_trigram_threshold_examples() {
        // prefix typing: 5/5 trigrams of "cmd_dup" in "cmd_duplicate_functions_with_count"
        let q = trigrams("cmd_dup");
        let score = trigram_containment(&q, "cmd_duplicate_functions_with_count");
        assert!(
            score >= 0.5,
            "cmd_dup vs cmd_duplicate_functions_with_count: {}",
            score
        );

        // short query skipped
        let result = suggest_symbols_trigram("abc", std::path::Path::new("."), 0.5, 5);
        assert!(
            result.is_empty(),
            "should skip queries shorter than 4 chars"
        );
    }

    #[test]
    fn test_suggest_symbols_finds_typo() {
        // Run from workspace root — find cmd_view_symbol_direct via typo query
        let root = std::env::current_dir()
            .unwrap()
            .ancestors()
            .find(|p| p.join("Cargo.toml").exists() && p.join("crates").exists())
            .unwrap_or(&std::env::current_dir().unwrap())
            .to_path_buf();
        let suggestions = suggest_symbols_trigram("xmd_view_smbl", &root, 0.5, 10);
        assert!(
            !suggestions.is_empty(),
            "expected at least one trigram suggestion for typo query"
        );
        let names: Vec<_> = suggestions.iter().map(|(s, _)| s.name.as_str()).collect();
        assert!(
            names.iter().any(|n| n.contains("cmd_view")),
            "expected a cmd_view_* symbol in suggestions, got: {:?}",
            names
        );
    }
}
