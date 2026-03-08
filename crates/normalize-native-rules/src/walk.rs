/// Shared file-walking utilities for analyze commands.
///
/// All walkers respect `.gitignore` (and nested `.gitignore` files in subdirectories)
/// via the `ignore` crate. A supplemental `is_excluded_dir` filter prunes well-known
/// dependency/build directories that may not appear in `.gitignore` (e.g. `vendor/`
/// committed in Go/PHP projects, `.venv` outside the repo root).
use std::path::Path;

/// Well-known dependency and build directories to skip during analysis.
///
/// This supplements `.gitignore`-based exclusion. If a directory is already covered
/// by `.gitignore`, the walker never descends into it regardless of this list.
/// Add entries here for directories that are sometimes committed or created outside
/// the repo's gitignore scope.
pub fn is_excluded_dir(name: &str) -> bool {
    matches!(
        name,
        // JavaScript / TypeScript
        "node_modules"
        // Rust
        | "target"
        // Go / PHP / Ruby
        | "vendor"
        // Python
        | "__pycache__" | ".venv" | "venv" | "env" | ".tox" | ".mypy_cache" | ".ruff_cache"
        // Java / Kotlin / Gradle / Maven
        | ".gradle" | ".m2"
        // Generic build outputs
        | "dist" | "build" | ".build" | "out" | "output"
        // VCS / tooling
        | ".git" | ".svn" | ".hg" | ".claude"
    ) || name.starts_with('.')
}

/// Build a gitignore-aware directory walker rooted at `root`.
///
/// - Respects `.gitignore`, `.git/info/exclude`, and global gitignore.
/// - Prunes `is_excluded_dir` directories early (no descent into them).
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
        .filter_entry(|e| !e.file_name().to_str().is_some_and(is_excluded_dir));
    builder.build().filter_map(|e| e.ok())
}
