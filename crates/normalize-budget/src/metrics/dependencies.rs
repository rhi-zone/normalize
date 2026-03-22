//! Dependency diff metric: entries added/removed from manifests.

use super::DiffMetric;
use std::path::Path;
use std::process::Command;

/// Dependencies added or removed from manifest files.
///
/// Watches `Cargo.toml`, `package.json`, `requirements.txt`, `pyproject.toml`,
/// `go.mod`, `pom.xml`, `build.gradle`, `*.gemspec`, and similar manifest files.
/// Returns `(manifest_file, deps_added, deps_removed)` by counting dependency-like
/// lines added/removed in the diff.
pub struct DependenciesMetric;

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

impl DiffMetric for DependenciesMetric {
    fn name(&self) -> &'static str {
        "dependencies"
    }

    fn measure_diff(&self, root: &Path, base_ref: &str) -> anyhow::Result<Vec<(String, f64, f64)>> {
        let output = Command::new("git")
            .args(["diff", base_ref, "--"])
            .current_dir(root)
            .output()
            .map_err(|e| anyhow::anyhow!("failed to run git diff: {e}"))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!("git diff failed: {stderr}"));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut file_added: std::collections::HashMap<String, f64> = Default::default();
        let mut file_removed: std::collections::HashMap<String, f64> = Default::default();
        let mut current_file = String::new();
        let mut in_manifest = false;

        for line in stdout.lines() {
            if let Some(rest) = line.strip_prefix("+++ b/") {
                current_file = rest.to_string();
                in_manifest = is_manifest(&current_file);
            } else if line.starts_with("--- ") || line.starts_with("diff --git") {
                // skip
            } else if in_manifest {
                if line.starts_with('+')
                    && !line.starts_with("+++")
                    && looks_like_dep_line(&line[1..])
                {
                    *file_added.entry(current_file.clone()).or_default() += 1.0;
                } else if line.starts_with('-')
                    && !line.starts_with("---")
                    && looks_like_dep_line(&line[1..])
                {
                    *file_removed.entry(current_file.clone()).or_default() += 1.0;
                }
            }
        }

        let mut all_files: std::collections::HashSet<String> = Default::default();
        all_files.extend(file_added.keys().cloned());
        all_files.extend(file_removed.keys().cloned());

        let results = all_files
            .into_iter()
            .filter(|f| !f.is_empty())
            .map(|f| {
                let added = file_added.get(&f).copied().unwrap_or(0.0);
                let removed = file_removed.get(&f).copied().unwrap_or(0.0);
                (f, added, removed)
            })
            .collect();

        Ok(results)
    }
}
