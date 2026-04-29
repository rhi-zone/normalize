/// Shared file-walking utilities for native rule checks.
///
/// All walkers respect `.gitignore` (and nested `.gitignore` files in subdirectories)
/// via the `ignore` crate. Directory exclusions and ignore-file selection are
/// configurable via [`WalkConfig`] (from `[walk]` in `.normalize/config.toml`).
use normalize_rules_config::{PathFilter, WalkConfig};
use std::path::Path;

/// Configure a [`ignore::WalkBuilder`] according to the given [`WalkConfig`].
///
/// - Enables/disables `.gitignore` based on `ignore_files`.
/// - Adds any additional ignore files via `WalkBuilder::add_ignore`.
/// - Applies `exclude` patterns in a `filter_entry` closure.
fn configure_walk_builder(
    builder: &mut ignore::WalkBuilder,
    walk_config: &WalkConfig,
    root: &Path,
) {
    let ignore_files = walk_config.ignore_files();
    let has_gitignore = ignore_files.contains(&".gitignore");

    builder.hidden(false);
    builder.git_ignore(has_gitignore);
    builder.git_global(has_gitignore);
    builder.git_exclude(has_gitignore);

    // Add any non-.gitignore ignore files
    for file in &ignore_files {
        if *file != ".gitignore" {
            let ignore_path = root.join(file);
            if ignore_path.exists() {
                builder.add_ignore(ignore_path);
            }
        }
    }
}

/// Build a directory walker rooted at `root`, configured by [`WalkConfig`].
///
/// - Respects ignore files as configured (default: `.gitignore`, `.git/info/exclude`,
///   and global gitignore).
/// - Skips directories matching `exclude` patterns (default: `.git`).
/// - Visits hidden files/directories (filtering delegated to ignore files and caller).
///
/// Returns a flat iterator of successfully-read `DirEntry` values.
pub fn gitignore_walk(
    root: &Path,
    walk_config: &WalkConfig,
) -> impl Iterator<Item = ignore::DirEntry> {
    let mut builder = ignore::WalkBuilder::new(root);
    configure_walk_builder(&mut builder, walk_config, root);
    // Compile gitignore-style exclude patterns once, anchored at root.
    let excludes = walk_config.compiled_excludes(root);
    let root_owned = root.to_path_buf();
    builder.filter_entry(move |e| {
        let path = e.path();
        let rel = path.strip_prefix(&root_owned).unwrap_or(path);
        // Empty rel (root itself) — never exclude.
        if rel.as_os_str().is_empty() {
            return true;
        }
        let is_dir = e.file_type().is_some_and(|ft| ft.is_dir());
        !excludes
            .matched_path_or_any_parents(rel, is_dir)
            .is_ignore()
    });
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
    walk_config: &'a WalkConfig,
) -> Box<dyn Iterator<Item = ignore::DirEntry> + 'a> {
    if filter.is_empty() {
        return Box::new(gitignore_walk(root, walk_config));
    }
    Box::new(gitignore_walk(root, walk_config).filter(move |entry| {
        // Always pass directories through — callers may need to descend.
        if entry.file_type().is_some_and(|ft| ft.is_dir()) {
            return true;
        }
        let rel = entry.path().strip_prefix(root).unwrap_or(entry.path());
        filter.matches_path(rel)
    }))
}
