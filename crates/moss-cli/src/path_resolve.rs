use ignore::WalkBuilder;
use nucleo_matcher::pattern::{CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Config, Matcher};
use std::path::Path;

use crate::index::FileIndex;

#[derive(Debug, Clone)]
pub struct PathMatch {
    pub path: String,
    pub kind: String,
    pub score: u32,
}

/// Get all files in the repository (uses index if available)
pub fn all_files(root: &Path) -> Vec<PathMatch> {
    get_all_paths(root)
        .into_iter()
        .map(|(path, is_dir)| PathMatch {
            path,
            kind: if is_dir { "directory" } else { "file" }.to_string(),
            score: 0,
        })
        .collect()
}

/// Resolve a fuzzy query to matching paths.
///
/// Handles:
/// - Exact paths: src/moss/dwim.py
/// - Partial filenames: dwim.py, dwim
/// - Directory names: moss, src
pub fn resolve(query: &str, root: &Path) -> Vec<PathMatch> {
    // Handle file:symbol syntax (defer symbol resolution to Python for now)
    if query.contains(':') {
        let file_part = query.split(':').next().unwrap();
        return resolve(file_part, root);
    }

    // Try to use index if available
    let all_paths = get_all_paths(root);

    resolve_from_paths(query, &all_paths)
}

/// Get all paths, using index if available, falling back to filesystem walk
fn get_all_paths(root: &Path) -> Vec<(String, bool)> {
    // Try index first
    if let Ok(mut index) = FileIndex::open(root) {
        if index.needs_refresh() {
            let _ = index.refresh();
        }
        if let Ok(files) = index.all_files() {
            return files.into_iter().map(|f| (f.path, f.is_dir)).collect();
        }
    }

    // Fall back to filesystem walk
    let mut all_paths: Vec<(String, bool)> = Vec::new();
    let walker = WalkBuilder::new(root)
        .hidden(false)
        .git_ignore(true)
        .git_global(true)
        .git_exclude(true)
        .build();

    for entry in walker.flatten() {
        let path = entry.path();
        if let Ok(rel) = path.strip_prefix(root) {
            let rel_str = rel.to_string_lossy().to_string();
            if !rel_str.is_empty() {
                let is_dir = path.is_dir();
                all_paths.push((rel_str, is_dir));
            }
        }
    }

    all_paths
}

/// Normalize a char for comparison
#[inline]
fn normalize_char(c: char) -> char {
    match c {
        '-' | '.' | '_' => ' ',
        c => c.to_ascii_lowercase(),
    }
}

/// Compare two strings with normalization (no allocation)
fn eq_normalized(a: &str, b: &str) -> bool {
    let mut a_chars = a.chars().map(normalize_char);
    let mut b_chars = b.chars().map(normalize_char);
    loop {
        match (a_chars.next(), b_chars.next()) {
            (Some(ac), Some(bc)) if ac == bc => continue,
            (None, None) => return true,
            _ => return false,
        }
    }
}

/// Normalize string for comparison (used for filename matching)
fn normalize_for_match(s: &str) -> String {
    s.chars().map(normalize_char).collect()
}

/// Resolve from a pre-loaded list of paths
fn resolve_from_paths(query: &str, all_paths: &[(String, bool)]) -> Vec<PathMatch> {
    let query_lower = query.to_lowercase();
    let query_normalized = normalize_for_match(query);

    // Try normalized path match (handles exact match too, no allocation)
    for (path, is_dir) in all_paths {
        if eq_normalized(path, query) {
            return vec![PathMatch {
                path: path.clone(),
                kind: if *is_dir { "directory" } else { "file" }.to_string(),
                score: u32::MAX,
            }];
        }
    }

    // Try exact filename/dirname match (case-insensitive, _ and - equivalent)
    let mut exact_matches: Vec<PathMatch> = Vec::new();
    for (path, is_dir) in all_paths {
        let name = Path::new(path)
            .file_name()
            .map(|n| n.to_string_lossy().to_lowercase())
            .unwrap_or_default();
        let stem = Path::new(path)
            .file_stem()
            .map(|n| n.to_string_lossy().to_lowercase())
            .unwrap_or_default();
        let name_normalized = normalize_for_match(&name);
        let stem_normalized = normalize_for_match(&stem);

        if name == query_lower || stem == query_lower
            || name_normalized == query_normalized || stem_normalized == query_normalized {
            exact_matches.push(PathMatch {
                path: path.clone(),
                kind: if *is_dir { "directory" } else { "file" }.to_string(),
                score: u32::MAX - 1,
            });
        }
    }

    if !exact_matches.is_empty() {
        return exact_matches;
    }

    // Fuzzy match using nucleo
    let mut matcher = Matcher::new(Config::DEFAULT);
    let pattern = Pattern::parse(query, CaseMatching::Ignore, Normalization::Smart);

    let mut fuzzy_matches: Vec<PathMatch> = Vec::new();

    for (path, is_dir) in all_paths {
        let mut buf = Vec::new();
        if let Some(score) = pattern.score(nucleo_matcher::Utf32Str::new(path, &mut buf), &mut matcher) {
            fuzzy_matches.push(PathMatch {
                path: path.clone(),
                kind: if *is_dir { "directory" } else { "file" }.to_string(),
                score,
            });
        }
    }

    // Sort by score descending, take top 10
    fuzzy_matches.sort_by(|a, b| b.score.cmp(&a.score));
    fuzzy_matches.truncate(10);

    fuzzy_matches
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_exact_match() {
        let dir = tempdir().unwrap();
        fs::create_dir_all(dir.path().join("src/moss")).unwrap();
        fs::write(dir.path().join("src/moss/cli.py"), "").unwrap();

        let matches = resolve("src/moss/cli.py", dir.path());
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].path, "src/moss/cli.py");
    }

    #[test]
    fn test_filename_match() {
        let dir = tempdir().unwrap();
        fs::create_dir_all(dir.path().join("src/moss")).unwrap();
        fs::write(dir.path().join("src/moss/dwim.py"), "").unwrap();

        let matches = resolve("dwim.py", dir.path());
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].path, "src/moss/dwim.py");
    }

    #[test]
    fn test_stem_match() {
        let dir = tempdir().unwrap();
        fs::create_dir_all(dir.path().join("src/moss")).unwrap();
        fs::write(dir.path().join("src/moss/dwim.py"), "").unwrap();

        let matches = resolve("dwim", dir.path());
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].path, "src/moss/dwim.py");
    }

    #[test]
    fn test_underscore_hyphen_equivalence() {
        let dir = tempdir().unwrap();
        fs::create_dir_all(dir.path().join("docs")).unwrap();
        fs::write(dir.path().join("docs/prior-art.md"), "").unwrap();

        // underscore query should match hyphen filename
        let matches = resolve("prior_art", dir.path());
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].path, "docs/prior-art.md");

        // hyphen query should also work
        let matches = resolve("prior-art", dir.path());
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].path, "docs/prior-art.md");

        // full path with underscores should match hyphenated path
        let matches = resolve("docs/prior_art.md", dir.path());
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].path, "docs/prior-art.md");
    }
}
