//! Dependency diff metric: entries added/removed from manifests.

use super::{DiffMeasurement, DiffMetric};
use crate::git_ops;
use std::path::Path;

/// Dependencies added or removed from manifest files.
///
/// Watches `Cargo.toml`, `package.json`, `requirements.txt`, `pyproject.toml`,
/// `go.mod`, `pom.xml`, `build.gradle`, `*.gemspec`, and similar manifest files.
/// Returns measurements with `(manifest_file, deps_added, deps_removed)` by counting
/// dependency-like lines added/removed between `base_ref` and HEAD.
pub struct DependencyDeltaMetric;

/// Returns true if a file path looks like a dependency manifest.
fn is_manifest(path: &str) -> bool {
    let name = std::path::Path::new(path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("");
    matches!(
        name,
        "Cargo.toml"
            | "package.json"
            | "requirements.txt"
            | "pyproject.toml"
            | "go.mod"
            | "pom.xml"
            | "build.gradle"
            | "build.gradle.kts"
            | "Gemfile"
            | "composer.json"
    )
}

/// Heuristic: is this line in a manifest likely a dependency entry?
fn looks_like_dep_line(line: &str) -> bool {
    let t = line.trim();
    // Cargo.toml: `name = "..."`  or `name = { ... }`
    // package.json: `"name": "..."` in a deps block
    // requirements.txt: package lines
    // go.mod: `require` lines
    // We count any non-empty, non-comment, non-section-header line as a dep candidate
    // when in a manifest file. This is intentionally loose — exact dep parsing would
    // require per-format logic.
    !t.is_empty()
        && !t.starts_with('#')
        && !t.starts_with('[')
        && !t.starts_with('{')
        && !t.starts_with('}')
        && !t.starts_with("//")
        && !t.starts_with("/*")
        && t != "dependencies" // go.mod
}

/// Count dependency-like lines in a manifest file's content.
fn count_dep_lines(content: &str) -> usize {
    content
        .lines()
        .filter(|line| looks_like_dep_line(line))
        .count()
}

impl DiffMetric for DependencyDeltaMetric {
    fn name(&self) -> &'static str {
        "dependencies"
    }

    fn measure_diff(&self, root: &Path, base_ref: &str) -> anyhow::Result<Vec<DiffMeasurement>> {
        let repo = git_ops::open_repo(root)?;
        let changes = git_ops::diff_base_to_head(root, base_ref)?;

        let mut results = Vec::new();

        for change in changes {
            if !is_manifest(&change.path) {
                continue;
            }

            let old_count = change
                .old_id
                .and_then(|id| git_ops::read_blob_text(&repo, id))
                .map(|c| count_dep_lines(&c))
                .unwrap_or(0);

            let new_count = change
                .new_id
                .and_then(|id| git_ops::read_blob_text(&repo, id))
                .map(|c| count_dep_lines(&c))
                .unwrap_or(0);

            let added = new_count.saturating_sub(old_count) as f64;
            let removed = old_count.saturating_sub(new_count) as f64;

            if added > 0.0 || removed > 0.0 {
                results.push(DiffMeasurement {
                    key: change.path,
                    added,
                    removed,
                });
            }
        }

        Ok(results)
    }
}
