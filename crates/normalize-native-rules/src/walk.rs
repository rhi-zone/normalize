/// Shared file-walking utilities for native rule checks.
///
/// All walkers respect `.gitignore` (and nested `.gitignore` files in subdirectories)
/// via the `ignore` crate. The only hardcoded exclusion is `.git` itself — everything
/// else is delegated to `.gitignore`.
use normalize_rules_config::PathFilter;
use std::path::Path;

/// Build a gitignore-aware directory walker rooted at `root`.
///
/// - Respects `.gitignore`, `.git/info/exclude`, and global gitignore.
/// - Skips `.git` directories (never useful to scan).
/// - Visits hidden files/directories (filtering delegated to gitignore and caller).
///
/// Returns a flat iterator of successfully-read `DirEntry` values.
pub fn gitignore_walk(root: &Path) -> impl Iterator<Item = ignore::DirEntry> {
    let mut builder = ignore::WalkBuilder::new(root);
    builder
        .hidden(false)
        .git_ignore(true)
        .git_global(true)
        .git_exclude(true)
        .filter_entry(|e| e.file_name() != ".git");
    builder.build().filter_map(|e| e.ok())
}

/// Like [`gitignore_walk`], but additionally filters file entries through a [`PathFilter`].
///
/// Directory entries are always passed through (so the walker can descend into them).
/// Only file entries are tested against `--only` / `--exclude` patterns using
/// their path relative to `root`.
#[allow(dead_code)]
pub fn filtered_gitignore_walk<'a>(
    root: &'a Path,
    filter: &'a PathFilter,
) -> Box<dyn Iterator<Item = ignore::DirEntry> + 'a> {
    if filter.is_empty() {
        return Box::new(gitignore_walk(root));
    }
    Box::new(gitignore_walk(root).filter(move |entry| {
        // Always pass directories through — callers may need to descend.
        if entry.file_type().is_some_and(|ft| ft.is_dir()) {
            return true;
        }
        let rel = entry.path().strip_prefix(root).unwrap_or(entry.path());
        filter.matches_path(rel)
    }))
}
