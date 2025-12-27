//! Shadow Git: hunk-level edit tracking with rollback support.
//!
//! Uses a separate git repository in `.moss/.git` to track edits without
//! polluting the user's repository. Provides atomic snapshots and
//! granular rollback at the hunk level.
//!
//! ## Integration
//!
//! All edits made via `moss edit` or workflow edit tools should auto-snapshot
//! before applying changes. This enables:
//! - Hunk-level rollback of bad edits
//! - Detection of suspicious deletions (high deletion_ratio)
//! - Full audit trail of agent modifications

use std::cell::RefCell;
use std::path::{Path, PathBuf};

use git2::{DiffOptions, Repository, Signature};

/// A single hunk from a diff.
#[derive(Debug, Clone)]
pub struct Hunk {
    pub id: usize,
    pub file: PathBuf,
    pub old_start: u32,
    pub old_lines: u32,
    pub new_start: u32,
    pub new_lines: u32,
    pub header: String,
    pub content: String,
}

impl Hunk {
    /// Returns true if this hunk only removes lines.
    pub fn is_pure_deletion(&self) -> bool {
        self.old_lines > 0 && self.new_lines == 0
    }

    /// Ratio of deleted to added lines. High ratio = suspicious.
    pub fn deletion_ratio(&self) -> f32 {
        if self.new_lines == 0 {
            self.old_lines as f32
        } else {
            self.old_lines as f32 / self.new_lines as f32
        }
    }
}

/// Shadow git repository for tracking edits.
pub struct ShadowGit {
    repo: Repository,
    root: PathBuf,
}

/// Snapshot identifier (git commit OID as hex string).
pub type SnapshotId = String;

impl ShadowGit {
    /// Initialize or open shadow git in `.moss/.git` under the given root.
    pub fn open(root: &Path) -> Result<Self, git2::Error> {
        let shadow_path = root.join(".moss");
        std::fs::create_dir_all(&shadow_path).ok();

        let repo = match Repository::open(&shadow_path) {
            Ok(r) => r,
            Err(_) => {
                let repo = Repository::init(&shadow_path)?;
                // Create initial empty commit
                let sig = Signature::now("moss", "moss@localhost")?;
                let tree_id = repo.index()?.write_tree()?;
                {
                    let tree = repo.find_tree(tree_id)?;
                    repo.commit(Some("HEAD"), &sig, &sig, "initial", &tree, &[])?;
                }
                repo
            }
        };

        Ok(Self {
            repo,
            root: root.to_path_buf(),
        })
    }

    /// Take a snapshot of the given files. Returns snapshot ID.
    pub fn snapshot(&self, files: &[&Path]) -> Result<SnapshotId, git2::Error> {
        let mut index = self.repo.index()?;

        for file in files {
            let rel_path = file.strip_prefix(&self.root).unwrap_or(file);
            let abs_path = self.root.join(rel_path);

            if abs_path.exists() {
                // Read file content and add to index
                let content = std::fs::read(&abs_path).unwrap_or_default();
                let blob_oid = self.repo.blob(&content)?;

                let entry = git2::IndexEntry {
                    ctime: git2::IndexTime::new(0, 0),
                    mtime: git2::IndexTime::new(0, 0),
                    dev: 0,
                    ino: 0,
                    mode: 0o100644,
                    uid: 0,
                    gid: 0,
                    file_size: content.len() as u32,
                    id: blob_oid,
                    flags: 0,
                    flags_extended: 0,
                    path: rel_path.to_string_lossy().into_owned().into_bytes(),
                };
                index.add(&entry)?;
            } else {
                // File deleted - remove from index
                let _ = index.remove_path(rel_path);
            }
        }

        index.write()?;
        let tree_id = index.write_tree()?;
        let tree = self.repo.find_tree(tree_id)?;

        let sig = Signature::now("moss", "moss@localhost")?;
        let parent = self.repo.head()?.peel_to_commit()?;

        let oid = self.repo.commit(
            Some("HEAD"),
            &sig,
            &sig,
            &format!("snapshot: {} files", files.len()),
            &tree,
            &[&parent],
        )?;

        Ok(oid.to_string())
    }

    /// Get all hunks between a snapshot and current working directory state.
    pub fn hunks_since(&self, snapshot_id: &str) -> Result<Vec<Hunk>, git2::Error> {
        let oid = git2::Oid::from_str(snapshot_id)?;
        let commit = self.repo.find_commit(oid)?;
        let old_tree = commit.tree()?;

        // Build current tree from working directory
        let mut index = self.repo.index()?;
        index.read(true)?;
        let new_tree_id = index.write_tree()?;
        let new_tree = self.repo.find_tree(new_tree_id)?;

        let mut diff_opts = DiffOptions::new();
        let diff =
            self.repo
                .diff_tree_to_tree(Some(&old_tree), Some(&new_tree), Some(&mut diff_opts))?;

        let hunks = RefCell::new(Vec::new());
        let hunk_id = RefCell::new(0usize);
        let current_file = RefCell::new(PathBuf::new());

        diff.foreach(
            &mut |delta, _| {
                if let Some(path) = delta.new_file().path() {
                    *current_file.borrow_mut() = path.to_path_buf();
                }
                true
            },
            None,
            Some(&mut |_delta, hunk| {
                let mut id = hunk_id.borrow_mut();
                hunks.borrow_mut().push(Hunk {
                    id: *id,
                    file: current_file.borrow().clone(),
                    old_start: hunk.old_start(),
                    old_lines: hunk.old_lines(),
                    new_start: hunk.new_start(),
                    new_lines: hunk.new_lines(),
                    header: String::from_utf8_lossy(hunk.header()).to_string(),
                    content: String::new(),
                });
                *id += 1;
                true
            }),
            Some(&mut |_delta, _hunk, line| {
                if let Some(h) = hunks.borrow_mut().last_mut() {
                    let prefix = match line.origin() {
                        '+' => "+",
                        '-' => "-",
                        ' ' => " ",
                        _ => "",
                    };
                    h.content.push_str(&format!(
                        "{}{}",
                        prefix,
                        String::from_utf8_lossy(line.content())
                    ));
                }
                true
            }),
        )?;

        Ok(hunks.into_inner())
    }

    /// Get hunks between current HEAD and working directory.
    pub fn hunks(&self) -> Result<Vec<Hunk>, git2::Error> {
        let head = self.repo.head()?.peel_to_commit()?;
        self.hunks_since(&head.id().to_string())
    }

    /// Restore specific files to a snapshot state.
    pub fn restore(&self, snapshot_id: &str, files: Option<&[&Path]>) -> Result<(), git2::Error> {
        let oid = git2::Oid::from_str(snapshot_id)?;
        let commit = self.repo.find_commit(oid)?;
        let tree = commit.tree()?;

        match files {
            Some(paths) => {
                // Restore specific files
                for file in paths {
                    let rel_path = file.strip_prefix(&self.root).unwrap_or(file);
                    if let Ok(entry) = tree.get_path(rel_path) {
                        let blob = self.repo.find_blob(entry.id())?;
                        let abs_path = self.root.join(rel_path);
                        std::fs::write(&abs_path, blob.content()).ok();
                    }
                }
            }
            None => {
                // Restore all files in tree
                tree.walk(git2::TreeWalkMode::PreOrder, |dir, entry| {
                    if entry.kind() == Some(git2::ObjectType::Blob) {
                        let rel_path = if dir.is_empty() {
                            PathBuf::from(entry.name().unwrap_or(""))
                        } else {
                            PathBuf::from(dir).join(entry.name().unwrap_or(""))
                        };
                        if let Ok(blob) = self.repo.find_blob(entry.id()) {
                            let abs_path = self.root.join(&rel_path);
                            if let Some(parent) = abs_path.parent() {
                                std::fs::create_dir_all(parent).ok();
                            }
                            std::fs::write(&abs_path, blob.content()).ok();
                        }
                    }
                    git2::TreeWalkResult::Ok
                })?;
            }
        }

        Ok(())
    }

    /// Get the current HEAD snapshot ID.
    pub fn head(&self) -> Result<SnapshotId, git2::Error> {
        let head = self.repo.head()?.peel_to_commit()?;
        Ok(head.id().to_string())
    }

    /// List all snapshots (commits) with their messages.
    pub fn list_snapshots(&self) -> Result<Vec<(SnapshotId, String)>, git2::Error> {
        let mut revwalk = self.repo.revwalk()?;
        revwalk.push_head()?;

        let mut snapshots = Vec::new();
        for oid in revwalk {
            let oid = oid?;
            let commit = self.repo.find_commit(oid)?;
            let msg = commit.message().unwrap_or("").to_string();
            snapshots.push((oid.to_string(), msg));
        }

        Ok(snapshots)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_shadow_git_init() {
        let tmp = TempDir::new().unwrap();
        let shadow = ShadowGit::open(tmp.path()).unwrap();

        // Should have created .moss/.git
        assert!(tmp.path().join(".moss/.git").exists());

        // Should have a HEAD
        let head = shadow.head().unwrap();
        assert!(!head.is_empty());
    }

    #[test]
    fn test_snapshot_and_hunks() {
        let tmp = TempDir::new().unwrap();
        let shadow = ShadowGit::open(tmp.path()).unwrap();

        // Create a file
        let file_path = tmp.path().join("test.txt");
        fs::write(&file_path, "line1\nline2\n").unwrap();

        // Snapshot it
        let snap1 = shadow.snapshot(&[&file_path]).unwrap();

        // Modify the file
        fs::write(&file_path, "line1\nmodified\nline3\n").unwrap();

        // Snapshot again
        let _snap2 = shadow.snapshot(&[&file_path]).unwrap();

        // Get hunks since first snapshot
        let hunks = shadow.hunks_since(&snap1).unwrap();
        assert!(!hunks.is_empty());
    }

    #[test]
    fn test_restore() {
        let tmp = TempDir::new().unwrap();
        let shadow = ShadowGit::open(tmp.path()).unwrap();

        // Create and snapshot
        let file_path = tmp.path().join("test.txt");
        fs::write(&file_path, "original").unwrap();
        let snap1 = shadow.snapshot(&[&file_path]).unwrap();

        // Modify
        fs::write(&file_path, "modified").unwrap();
        let _snap2 = shadow.snapshot(&[&file_path]).unwrap();

        // Restore to snap1
        shadow.restore(&snap1, Some(&[&file_path])).unwrap();
        assert_eq!(fs::read_to_string(&file_path).unwrap(), "original");
    }

    #[test]
    fn test_hunk_deletion_detection() {
        let hunk = Hunk {
            id: 0,
            file: PathBuf::from("test.txt"),
            old_start: 1,
            old_lines: 5,
            new_start: 1,
            new_lines: 0,
            header: String::new(),
            content: String::new(),
        };

        assert!(hunk.is_pure_deletion());
        assert_eq!(hunk.deletion_ratio(), 5.0);
    }
}
