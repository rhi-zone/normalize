//! Git operations via `gix` — no PATH dependency on the `git` binary.
//!
//! All operations are read-only; they access the git object store directly.
//! Functions degrade gracefully (returning `Err` or empty results) if the
//! repository cannot be opened or a ref cannot be resolved.

use std::path::Path;

// ── Repository helpers ────────────────────────────────────────────────────────

/// Open the git repository at or containing `path`.
pub fn open_repo(path: &Path) -> anyhow::Result<gix::Repository> {
    gix::discover(path).map_err(|e| anyhow::anyhow!("not a git repository: {e}"))
}

/// Read the content of a blob by its object id as a `String`.
///
/// Returns `None` if the object cannot be read or is not valid UTF-8.
pub fn read_blob_text(repo: &gix::Repository, id: gix::hash::ObjectId) -> Option<String> {
    let obj = repo.find_object(id).ok()?;
    String::from_utf8(obj.data.clone()).ok()
}

// ── Tree walking at a ref ─────────────────────────────────────────────────────

/// Walk the tree at `git_ref` and call `visitor` for every blob (file).
///
/// `visitor` receives the repo-relative path and the object id of each file.
pub fn walk_tree_at_ref<F>(root: &Path, git_ref: &str, mut visitor: F) -> anyhow::Result<()>
where
    F: FnMut(&str, gix::hash::ObjectId),
{
    let repo = open_repo(root)?;
    let spec: &gix::bstr::BStr = git_ref.as_bytes().into();
    let id = repo
        .rev_parse_single(spec)
        .map_err(|e| anyhow::anyhow!("git ref '{git_ref}' not found: {e}"))?;

    let commit = id
        .object()
        .map_err(|e| anyhow::anyhow!("failed to read commit: {e}"))?
        .into_commit();
    let tree = commit
        .tree()
        .map_err(|e| anyhow::anyhow!("failed to read tree: {e}"))?;

    traverse_tree_entries(&repo, &tree, "", &mut visitor)?;
    Ok(())
}

/// Recursively walk tree entries, calling `visitor` for each blob.
fn traverse_tree_entries<F>(
    repo: &gix::Repository,
    tree: &gix::Tree<'_>,
    prefix: &str,
    visitor: &mut F,
) -> anyhow::Result<()>
where
    F: FnMut(&str, gix::hash::ObjectId),
{
    use gix::objs::tree::EntryKind;

    for entry_result in tree.iter() {
        let entry = entry_result.map_err(|e| anyhow::anyhow!("tree entry decode error: {e}"))?;
        let name = String::from_utf8_lossy(entry.inner.filename).into_owned();
        let full_path = if prefix.is_empty() {
            name
        } else {
            format!("{prefix}/{name}")
        };
        let oid = entry.inner.oid.to_owned();

        match entry.inner.mode.kind() {
            EntryKind::Blob | EntryKind::BlobExecutable => {
                visitor(&full_path, oid);
            }
            EntryKind::Tree => {
                let sub_obj = repo
                    .find_object(oid)
                    .map_err(|e| anyhow::anyhow!("failed to read sub-tree object: {e}"))?;
                let sub_tree = sub_obj.into_tree();
                traverse_tree_entries(repo, &sub_tree, &full_path, visitor)?;
            }
            _ => {} // symlinks (Link), submodules (Commit) — skip
        }
    }
    Ok(())
}
