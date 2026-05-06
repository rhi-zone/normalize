//! Sync command — copy a project (and its session metadata) to a destination.

use crate::output::OutputFormatter;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
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
    /// Number of files skipped because their content hash matched the manifest.
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
                let _ = writeln!(out, "  {} → {}", f.src, f.dest);
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

// ── Copy helpers ──────────────────────────────────────────────────────────────

/// Walk `src_root` and copy all non-excluded files to `dest_root`.
/// Returns (files_copied, file_items).
pub fn copy_tree(
    src_root: &Path,
    dest_root: &Path,
    dry_run: bool,
    verbose: bool,
    warnings: &mut Vec<String>,
    // normalize-syntax-allow: rust/tuple-return
) -> (usize, Vec<SyncFileItem>) {
    let mut count = 0usize;
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

        if verbose {
            items.push(SyncFileItem {
                src: src_path.to_string_lossy().into_owned(),
                dest: dest_path.to_string_lossy().into_owned(),
                excluded: false,
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
        count += 1;
    }

    (count, items)
}

// ── Sync manifest (incremental sync) ─────────────────────────────────────────

/// Manifest recording content hashes for all files copied in a previous sync.
/// Stored as `.normalize/sync-manifest.json` in the *destination* project root.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct SyncManifest {
    /// Map from relative path (slash-separated) to hex-encoded blake3 hash of source content.
    pub files: HashMap<String, String>,
}

impl SyncManifest {
    /// Load manifest from `<dest_root>/.normalize/sync-manifest.json`, or return empty.
    pub fn load(dest_root: &Path) -> Self {
        let path = dest_root.join(".normalize/sync-manifest.json");
        if let Ok(data) = std::fs::read_to_string(&path) {
            serde_json::from_str(&data).unwrap_or_default()
        } else {
            Self::default()
        }
    }

    /// Save manifest to `<dest_root>/.normalize/sync-manifest.json`.
    pub fn save(&self, dest_root: &Path) -> Result<(), String> {
        let dir = dest_root.join(".normalize");
        std::fs::create_dir_all(&dir).map_err(|e| format!("mkdir {}: {}", dir.display(), e))?;
        let path = dir.join("sync-manifest.json");
        let json =
            serde_json::to_string_pretty(self).map_err(|e| format!("serialize manifest: {}", e))?;
        std::fs::write(&path, json).map_err(|e| format!("write manifest {}: {}", path.display(), e))
    }
}

/// Hash a file's contents with blake3. Returns hex-encoded digest.
fn hash_file(path: &Path) -> Option<String> {
    let data = std::fs::read(path).ok()?;
    Some(blake3::hash(&data).to_hex().to_string())
}

/// Walk `src_root` and copy all non-excluded files to `dest_root`.
/// If `manifest` is Some, skips files whose content hash matches.
/// Returns (files_copied, files_unchanged, file_items, updated_manifest_entries).
#[allow(clippy::too_many_arguments)]
pub fn copy_tree_incremental(
    src_root: &Path,
    dest_root: &Path,
    dry_run: bool,
    verbose: bool,
    force: bool,
    manifest: &SyncManifest,
    warnings: &mut Vec<String>,
    // normalize-syntax-allow: rust/tuple-return
) -> (usize, usize, Vec<SyncFileItem>, HashMap<String, String>) {
    let mut copied = 0usize;
    let mut unchanged = 0usize;
    let mut items = Vec::new();
    let mut new_entries: HashMap<String, String> = HashMap::new();

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
        let rel_key = rel.to_string_lossy().replace('\\', "/");
        let dest_path = dest_root.join(rel);

        // Incremental: check hash unless --force
        if !force && !dry_run {
            if let Some(src_hash) = hash_file(src_path) {
                if manifest
                    .files
                    .get(&rel_key)
                    .map(|h| h == &src_hash)
                    .unwrap_or(false)
                {
                    unchanged += 1;
                    // Keep existing manifest entry
                    new_entries.insert(rel_key, src_hash);
                    continue;
                }
                // Will copy — record hash
                new_entries.insert(rel_key.clone(), src_hash);
            }
        }

        if verbose {
            items.push(SyncFileItem {
                src: src_path.to_string_lossy().into_owned(),
                dest: dest_path.to_string_lossy().into_owned(),
                excluded: false,
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

    (copied, unchanged, items, new_entries)
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
