//! Sync command — copy a project (and its session metadata) to a destination.

use crate::output::OutputFormatter;
use serde::Serialize;
use std::path::{Path, PathBuf};

// ── Report types ─────────────────────────────────────────────────────────────

/// Item describing a single file operation in a sync.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct SyncFileItem {
    /// Source path.
    pub src: String,
    /// Destination path.
    pub dest: String,
    /// Whether the file was skipped due to an exclude rule.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub excluded: bool,
    /// Whether the file was skipped because it was unchanged (incremental check).
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub unchanged: bool,
}

/// Report returned by `normalize sync`.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct SyncReport {
    /// Destination root.
    pub dest: String,
    /// Source project root that was synced.
    pub source: String,
    /// Number of files copied (or that would be copied with --dry-run).
    pub files_copied: usize,
    /// Number of session metadata files copied.
    pub sessions_copied: usize,
    /// Number of files skipped because they were unchanged (mtime+size or checksum match).
    pub files_unchanged: usize,
    /// Whether path rewriting was performed on the index DB.
    pub index_paths_rewritten: bool,
    /// Whether this was a dry run (nothing written).
    pub dry_run: bool,
    /// Verbose file list — populated only when --verbose is passed.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub files: Vec<SyncFileItem>,
    /// Non-fatal warnings encountered during sync.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
}

impl OutputFormatter for SyncReport {
    fn format_text(&self) -> String {
        use std::fmt::Write as _;
        let mut out = String::new();
        let verb = if self.dry_run { "Would sync" } else { "Synced" };
        let _ = writeln!(out, "{} {} → {}", verb, self.source, self.dest);
        let _ = writeln!(out, "  Project files:  {}", self.files_copied);
        if self.files_unchanged > 0 {
            let _ = writeln!(out, "  Unchanged (skipped): {}", self.files_unchanged);
        }
        if self.sessions_copied > 0 {
            let _ = writeln!(out, "  Session files:  {}", self.sessions_copied);
        }
        if self.index_paths_rewritten && !self.dry_run {
            let _ = writeln!(out, "  Index paths rewritten for new location");
        }
        if !self.files.is_empty() {
            let _ = writeln!(out);
            for f in &self.files {
                if f.unchanged {
                    let _ = writeln!(out, "  [skip] {}", f.src);
                } else {
                    let _ = writeln!(out, "  {} → {}", f.src, f.dest);
                }
            }
        }
        for w in &self.warnings {
            let _ = writeln!(out, "warning: {}", w);
        }
        out
    }
}

// ── Exclude rules ─────────────────────────────────────────────────────────────

/// Default directories/files to exclude from the project copy.
const DEFAULT_EXCLUDES: &[&str] = &[
    "target",
    "node_modules",
    ".git/objects",
    ".normalize/findings-cache.sqlite",
    ".fastembed_cache",
];

fn is_excluded(path: &Path, root: &Path) -> bool {
    let Ok(rel) = path.strip_prefix(root) else {
        return false;
    };
    let rel_str = rel.to_string_lossy();
    for exc in DEFAULT_EXCLUDES {
        // Match if the relative path starts with the exclude pattern or equals it.
        if rel_str == *exc
            || rel_str.starts_with(&format!("{}/", exc))
            || rel_str.starts_with(&format!("{}\\", exc))
        {
            return true;
        }
    }
    false
}

// ── Incremental check helpers ─────────────────────────────────────────────────

/// Returns true if `dest` exists, has the same size as `src`, and its mtime >= src mtime.
/// Fast O(1) metadata-only check — no file reads required.
fn is_unchanged_mtime_size(src: &Path, dest: &Path) -> bool {
    let Ok(src_meta) = std::fs::metadata(src) else {
        return false;
    };
    let Ok(dest_meta) = std::fs::metadata(dest) else {
        return false;
    };
    if src_meta.len() != dest_meta.len() {
        return false;
    }
    let Ok(src_mtime) = src_meta.modified() else {
        return false;
    };
    let Ok(dest_mtime) = dest_meta.modified() else {
        return false;
    };
    dest_mtime >= src_mtime
}

/// Compute SHA-256 digest of a file's contents. Returns raw bytes.
fn sha256_file(path: &Path) -> Option<Vec<u8>> {
    use sha2::Digest as _;
    let data = std::fs::read(path).ok()?;
    let mut h = sha2::Sha256::new();
    h.update(&data);
    Some(h.finalize().to_vec())
}

/// Returns true if both files exist and their SHA-256 digests match.
fn is_unchanged_checksum(src: &Path, dest: &Path) -> bool {
    match (sha256_file(src), sha256_file(dest)) {
        (Some(s), Some(d)) => s == d,
        _ => false,
    }
}

// ── Copy helper ───────────────────────────────────────────────────────────────

/// Walk `src_root` and copy all non-excluded files to `dest_root`.
///
/// Incremental behaviour (skipped when `force` is true or during a dry-run preview):
/// - Default: skip a file if dest exists, `dest_mtime >= src_mtime`, and sizes match.
/// - With `checksum = true`: skip a file if dest exists and SHA-256 digests match.
///
/// Returns `(files_copied, files_unchanged, file_items)`.
#[allow(clippy::too_many_arguments)]
pub fn copy_tree_incremental(
    src_root: &Path,
    dest_root: &Path,
    dry_run: bool,
    verbose: bool,
    force: bool,
    checksum: bool,
    warnings: &mut Vec<String>,
    // normalize-syntax-allow: rust/tuple-return
) -> (usize, usize, Vec<SyncFileItem>) {
    let mut copied = 0usize;
    let mut unchanged = 0usize;
    let mut items = Vec::new();

    for entry in walkdir::WalkDir::new(src_root)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let src_path = entry.path();
        if src_path.is_dir() {
            continue;
        }
        if is_excluded(src_path, src_root) {
            continue;
        }

        let Ok(rel) = src_path.strip_prefix(src_root) else {
            continue;
        };
        let dest_path = dest_root.join(rel);

        // Incremental: skip unchanged files unless --force or --dry-run.
        // Dry-run still runs the check so it can report what *would* be skipped.
        let skip = !force && {
            if checksum {
                is_unchanged_checksum(src_path, &dest_path)
            } else {
                is_unchanged_mtime_size(src_path, &dest_path)
            }
        };

        if skip {
            unchanged += 1;
            if verbose {
                items.push(SyncFileItem {
                    src: src_path.to_string_lossy().into_owned(),
                    dest: dest_path.to_string_lossy().into_owned(),
                    excluded: false,
                    unchanged: true,
                });
            }
            continue;
        }

        if verbose {
            items.push(SyncFileItem {
                src: src_path.to_string_lossy().into_owned(),
                dest: dest_path.to_string_lossy().into_owned(),
                excluded: false,
                unchanged: false,
            });
        }

        if !dry_run {
            if let Some(parent) = dest_path.parent()
                && let Err(e) = std::fs::create_dir_all(parent)
            {
                warnings.push(format!("mkdir {}: {}", parent.display(), e));
                continue;
            }
            if let Err(e) = std::fs::copy(src_path, &dest_path) {
                warnings.push(format!("copy {}: {}", src_path.display(), e));
                continue;
            }
        }
        copied += 1;
    }

    (copied, unchanged, items)
}

/// Discover session metadata roots for a project across all registered AI agent formats.
///
/// Delegates to the `normalize_chat_sessions` format registry, which covers Claude Code,
/// OpenAI Codex, Gemini CLI, and any other registered format. Only directories that
/// exist on disk are returned.
///
/// Prefer calling `normalize_chat_sessions::project_metadata_roots` directly where possible.
pub fn session_metadata_roots(project_root: &Path) -> Vec<PathBuf> {
    normalize_chat_sessions::project_metadata_roots(project_root)
}

// ── Index path rewriting ──────────────────────────────────────────────────────

/// Rewrite absolute paths in the copied index DB from `old_root` → `new_root`.
/// Uses libsql to run UPDATE statements on all tables that store file paths.
pub async fn rewrite_index_paths(
    db_path: &Path,
    old_root: &str,
    new_root: &str,
) -> Result<(), String> {
    let db = libsql::Builder::new_local(db_path)
        .build()
        .await
        .map_err(|e| format!("Failed to open index DB: {}", e))?;
    let conn = db
        .connect()
        .map_err(|e| format!("Failed to connect to index DB: {}", e))?;

    // Tables and columns that store absolute file paths.
    let updates: &[(&str, &str)] = &[
        ("files", "path"),
        ("symbols", "file"),
        ("calls", "file"),
        ("imports", "file"),
    ];

    for (table, col) in updates {
        let sql = format!("UPDATE {} SET {} = replace({}, ?, ?)", table, col, col);
        // Ignore errors for tables that may not exist in all index versions.
        let _ = conn
            .execute(&sql, libsql::params![old_root, new_root])
            .await;
    }

    Ok(())
}

// ── Project discovery (for --all) ─────────────────────────────────────────────

/// Decode a Claude Code mangled dir name back to a filesystem path.
/// `-home-user-git-foo` → `/home/user/git/foo`
fn decode_claude_dir_name(name: &str) -> PathBuf {
    let decoded = name.trim_start_matches('-').replace('-', "/");
    PathBuf::from(format!("/{}", decoded))
}

/// List all project roots known from Claude Code session metadata.
pub fn list_all_known_project_roots() -> Vec<PathBuf> {
    let home = match std::env::var("HOME") {
        Ok(h) => h,
        Err(_) => return Vec::new(),
    };
    let projects_dir = PathBuf::from(home).join(".claude/projects");
    if !projects_dir.exists() {
        return Vec::new();
    }
    let mut roots = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&projects_dir) {
        for entry in entries.filter_map(|e| e.ok()) {
            let dir_name = entry.file_name().to_string_lossy().into_owned();
            let candidate = decode_claude_dir_name(&dir_name);
            if candidate.exists() {
                roots.push(candidate);
            }
        }
    }
    roots
}

/// Return the common prefix of a set of paths, if one exists.
pub fn common_prefix(paths: &[PathBuf]) -> Option<PathBuf> {
    if paths.is_empty() {
        return None;
    }
    let mut prefix: Vec<&std::ffi::OsStr> = paths[0].components().map(|c| c.as_os_str()).collect();
    for p in paths.iter().skip(1) {
        let comps: Vec<&std::ffi::OsStr> = p.components().map(|c| c.as_os_str()).collect();
        let shared = prefix
            .iter()
            .zip(comps.iter())
            .take_while(|(a, b)| a == b)
            .count();
        prefix.truncate(shared);
    }
    if prefix.is_empty() {
        None
    } else {
        Some(prefix.iter().collect())
    }
}
