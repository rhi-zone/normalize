//! Import path query: find the shortest import chain between two files.

use crate::index::FileIndex;
use normalize_output::OutputFormatter;
use std::path::PathBuf;

/// Report for `normalize view import-path`.
#[derive(serde::Serialize, schemars::JsonSchema)]
pub struct ImportPathReport {
    /// The source file (root-relative).
    pub from: String,
    /// The target file (root-relative).
    pub to: String,
    /// Found import paths.  Empty when no path exists.
    /// Each inner vec is one path: [from, hop1, ..., to].
    pub paths: Vec<Vec<String>>,
    /// True when `--all` was requested (multiple paths may be present).
    pub all_paths: bool,
}

impl OutputFormatter for ImportPathReport {
    fn format_text(&self) -> String {
        if self.from == self.to {
            return "Same file".to_string();
        }
        if self.paths.is_empty() {
            return format!("No import path found between {} and {}", self.from, self.to);
        }
        let mut lines = Vec::new();
        for (i, path) in self.paths.iter().enumerate() {
            if self.all_paths && self.paths.len() > 1 {
                lines.push(format!("Path {}:", i + 1));
                lines.push(format!("  {}", path.join(" → ")));
            } else {
                lines.push(path.join(" → "));
            }
        }
        lines.join("\n")
    }
}

/// Resolve a user-supplied path to a root-relative string for DB lookup.
///
/// Accepts either an absolute path or a path relative to `root`.
/// Strips the root prefix and normalizes separators.
fn resolve_db_path(input: &str, root: &std::path::Path) -> String {
    let p = PathBuf::from(input);
    let abs = if p.is_absolute() { p } else { root.join(p) };
    // Strip root prefix to get root-relative string
    abs.strip_prefix(root)
        .map(|r| r.to_string_lossy().into_owned())
        .unwrap_or_else(|_| input.to_string())
}

/// Find import path(s) between `from_file` and `to_file`.
pub async fn find_import_path_command(
    idx: &FileIndex,
    root: &std::path::Path,
    from_file: &str,
    to_file: &str,
    all_paths: bool,
    path_limit: usize,
    reverse: bool,
) -> Result<ImportPathReport, libsql::Error> {
    const MAX_DEPTH: usize = 10;

    let (from_raw, to_raw) = if reverse {
        (
            resolve_db_path(to_file, root),
            resolve_db_path(from_file, root),
        )
    } else {
        (
            resolve_db_path(from_file, root),
            resolve_db_path(to_file, root),
        )
    };

    let paths = idx
        .find_import_path(&from_raw, &to_raw, all_paths, path_limit, MAX_DEPTH)
        .await?;

    let (report_from, report_to) = if reverse {
        (to_raw, from_raw)
    } else {
        (from_raw, to_raw)
    };

    Ok(ImportPathReport {
        from: report_from,
        to: report_to,
        paths,
        all_paths,
    })
}
