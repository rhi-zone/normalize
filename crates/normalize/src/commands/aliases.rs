//! Project language detection for filter aliases (`--exclude`/`--only`).

use std::path::Path;

/// Detect programming languages in the project.
pub fn detect_project_languages(root: &Path) -> Vec<String> {
    use std::collections::HashSet;

    let mut languages = HashSet::new();

    // Walk the project directory (limited depth for performance)
    let walker = ignore::WalkBuilder::new(root)
        .max_depth(Some(5))
        .hidden(false) // Include hidden directories
        .git_ignore(true)
        .build();

    for entry in walker.flatten() {
        let path = entry.path();
        if let Some(lang) = normalize_languages::support_for_path(path) {
            languages.insert(lang.name().to_string());
        }
    }

    let mut result: Vec<_> = languages.into_iter().collect();
    result.sort();
    result
}
