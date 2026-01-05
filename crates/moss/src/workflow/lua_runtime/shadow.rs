//! Shadow git and worktree Lua bindings for the workflow runtime.

use std::path::Path;
use std::sync::{Arc, Mutex};

use mlua::{FromLua, Lua, Result as LuaResult, Table, UserData, Value};

use crate::workflow::shadow::{Hunk, ShadowGit, ShadowWorktree};

/// Wrapper for ShadowGit to store in Lua registry.
pub(super) struct LuaShadowGit(Arc<ShadowGit>);

impl Clone for LuaShadowGit {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl UserData for LuaShadowGit {}

impl FromLua for LuaShadowGit {
    fn from_lua(value: Value, _lua: &Lua) -> LuaResult<Self> {
        match value {
            Value::UserData(ud) => Ok(ud.borrow::<LuaShadowGit>()?.clone()),
            _ => Err(mlua::Error::FromLuaConversionError {
                from: value.type_name(),
                to: "LuaShadowGit".to_string(),
                message: Some("expected ShadowGit userdata".to_string()),
            }),
        }
    }
}

/// Wrapper for ShadowWorktree to store in Lua registry.
pub(super) struct LuaShadowWorktree(Arc<Mutex<ShadowWorktree>>);

impl Clone for LuaShadowWorktree {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl UserData for LuaShadowWorktree {}

impl FromLua for LuaShadowWorktree {
    fn from_lua(value: Value, _lua: &Lua) -> LuaResult<Self> {
        match value {
            Value::UserData(ud) => Ok(ud.borrow::<LuaShadowWorktree>()?.clone()),
            _ => Err(mlua::Error::FromLuaConversionError {
                from: value.type_name(),
                to: "LuaShadowWorktree".to_string(),
                message: Some("expected ShadowWorktree userdata".to_string()),
            }),
        }
    }
}

/// Convert a slice of hunks to a Lua table.
fn hunks_to_lua_table(lua: &Lua, hunks: &[Hunk]) -> LuaResult<Table> {
    let result = lua.create_table()?;
    for (i, hunk) in hunks.iter().enumerate() {
        let h = lua.create_table()?;
        h.set("id", hunk.id)?;
        h.set("file", hunk.file.to_string_lossy().to_string())?;
        h.set("old_start", hunk.old_start)?;
        h.set("old_lines", hunk.old_lines)?;
        h.set("new_start", hunk.new_start)?;
        h.set("new_lines", hunk.new_lines)?;
        h.set("header", hunk.header.clone())?;
        h.set("content", hunk.content.clone())?;
        h.set("is_deletion", hunk.is_pure_deletion())?;
        h.set("deletion_ratio", hunk.deletion_ratio())?;
        result.set(i + 1, h)?;
    }
    Ok(result)
}

/// Register shadow git and worktree bindings in Lua globals.
pub(super) fn register(lua: &Lua, globals: &Table, root: &Path) -> LuaResult<()> {
    let shadow_table = lua.create_table()?;
    let root_path = root.to_path_buf();

    // shadow.open() -> initializes/opens shadow git, returns snapshot id
    let root_clone = root_path.clone();
    shadow_table.set(
        "open",
        lua.create_function(move |lua, ()| {
            let sg = ShadowGit::open(&root_clone).map_err(mlua::Error::external)?;
            let head = sg.head().map_err(mlua::Error::external)?;
            // Store in registry for later use
            lua.set_named_registry_value("_shadow_git", LuaShadowGit(Arc::new(sg)))?;
            Ok(head)
        })?,
    )?;

    // shadow.snapshot(files) -> snapshot id
    shadow_table.set(
        "snapshot",
        lua.create_function(|lua, files: Vec<String>| {
            let sg: LuaShadowGit = lua.named_registry_value("_shadow_git")?;
            let paths: Vec<std::path::PathBuf> =
                files.iter().map(std::path::PathBuf::from).collect();
            let refs: Vec<&Path> = paths.iter().map(|p| p.as_path()).collect();
            let id = sg.0.snapshot(&refs).map_err(mlua::Error::external)?;
            Ok(id)
        })?,
    )?;

    // shadow.hunks() -> table of hunks
    shadow_table.set(
        "hunks",
        lua.create_function(|lua, ()| {
            let sg: LuaShadowGit = lua.named_registry_value("_shadow_git")?;
            let hunks = sg.0.hunks().map_err(mlua::Error::external)?;
            hunks_to_lua_table(lua, &hunks)
        })?,
    )?;

    // shadow.hunks_since(snapshot_id) -> table of hunks
    shadow_table.set(
        "hunks_since",
        lua.create_function(|lua, snapshot_id: String| {
            let sg: LuaShadowGit = lua.named_registry_value("_shadow_git")?;
            let hunks =
                sg.0.hunks_since(&snapshot_id)
                    .map_err(mlua::Error::external)?;
            hunks_to_lua_table(lua, &hunks)
        })?,
    )?;

    // shadow.restore(snapshot_id, files?) -> restores files to snapshot state
    shadow_table.set(
        "restore",
        lua.create_function(|lua, (snapshot_id, files): (String, Option<Vec<String>>)| {
            let sg: LuaShadowGit = lua.named_registry_value("_shadow_git")?;
            let file_refs = files.as_ref().map(|f| {
                let paths: Vec<std::path::PathBuf> =
                    f.iter().map(std::path::PathBuf::from).collect();
                paths
            });
            let refs: Option<Vec<&Path>> = file_refs
                .as_ref()
                .map(|p| p.iter().map(|x| x.as_path()).collect());
            sg.0.restore(&snapshot_id, refs.as_deref())
                .map_err(mlua::Error::external)?;
            Ok(())
        })?,
    )?;

    // shadow.head() -> current snapshot id
    shadow_table.set(
        "head",
        lua.create_function(|lua, ()| {
            let sg: LuaShadowGit = lua.named_registry_value("_shadow_git")?;
            let head = sg.0.head().map_err(mlua::Error::external)?;
            Ok(head)
        })?,
    )?;

    // shadow.list() -> list of all snapshots
    shadow_table.set(
        "list",
        lua.create_function(|lua, ()| {
            let sg: LuaShadowGit = lua.named_registry_value("_shadow_git")?;
            let snapshots = sg.0.list_snapshots().map_err(mlua::Error::external)?;

            let result = lua.create_table()?;
            for (i, (id, msg)) in snapshots.iter().enumerate() {
                let s = lua.create_table()?;
                s.set("id", id.clone())?;
                s.set("message", msg.clone())?;
                result.set(i + 1, s)?;
            }
            Ok(result)
        })?,
    )?;

    // shadow.worktree subtable for isolated editing
    let worktree_table = lua.create_table()?;
    let root_for_worktree = root_path.clone();

    // shadow.worktree.open() -> opens/creates worktree, returns path
    worktree_table.set(
        "open",
        lua.create_function(move |lua, ()| {
            let wt = ShadowWorktree::open(&root_for_worktree).map_err(mlua::Error::external)?;
            let path = wt.path().to_string_lossy().to_string();
            lua.set_named_registry_value(
                "_shadow_worktree",
                LuaShadowWorktree(Arc::new(Mutex::new(wt))),
            )?;
            Ok(path)
        })?,
    )?;

    // shadow.worktree.sync() -> reset worktree to HEAD
    worktree_table.set(
        "sync",
        lua.create_function(|lua, ()| {
            let wt: LuaShadowWorktree = lua.named_registry_value("_shadow_worktree")?;
            wt.0.lock()
                .map_err(|_| mlua::Error::external("lock poisoned"))?
                .sync()
                .map_err(mlua::Error::external)?;
            Ok(())
        })?,
    )?;

    // shadow.worktree.edit(path, content) -> edit file in worktree
    worktree_table.set(
        "edit",
        lua.create_function(|lua, (path, content): (String, String)| {
            let wt: LuaShadowWorktree = lua.named_registry_value("_shadow_worktree")?;
            wt.0.lock()
                .map_err(|_| mlua::Error::external("lock poisoned"))?
                .edit(Path::new(&path), &content)
                .map_err(mlua::Error::external)?;
            Ok(())
        })?,
    )?;

    // shadow.worktree.read(path) -> read file from worktree
    worktree_table.set(
        "read",
        lua.create_function(|lua, path: String| {
            let wt: LuaShadowWorktree = lua.named_registry_value("_shadow_worktree")?;
            let content =
                wt.0.lock()
                    .map_err(|_| mlua::Error::external("lock poisoned"))?
                    .read(Path::new(&path))
                    .map_err(mlua::Error::external)?;
            Ok(content)
        })?,
    )?;

    // shadow.worktree.validate(cmd) -> run validation, returns {success, stdout, stderr}
    worktree_table.set(
        "validate",
        lua.create_function(|lua, cmd: String| {
            let wt: LuaShadowWorktree = lua.named_registry_value("_shadow_worktree")?;
            let result =
                wt.0.lock()
                    .map_err(|_| mlua::Error::external("lock poisoned"))?
                    .validate(&cmd)
                    .map_err(mlua::Error::external)?;

            let t = lua.create_table()?;
            t.set("success", result.success)?;
            t.set("stdout", result.stdout)?;
            t.set("stderr", result.stderr)?;
            t.set("exit_code", result.exit_code)?;
            Ok(t)
        })?,
    )?;

    // shadow.worktree.diff() -> get diff of changes
    worktree_table.set(
        "diff",
        lua.create_function(|lua, ()| {
            let wt: LuaShadowWorktree = lua.named_registry_value("_shadow_worktree")?;
            let diff =
                wt.0.lock()
                    .map_err(|_| mlua::Error::external("lock poisoned"))?
                    .diff()
                    .map_err(mlua::Error::external)?;
            Ok(diff)
        })?,
    )?;

    // shadow.worktree.apply() -> apply changes to real repo, returns list of files
    worktree_table.set(
        "apply",
        lua.create_function(|lua, ()| {
            let wt: LuaShadowWorktree = lua.named_registry_value("_shadow_worktree")?;
            let files =
                wt.0.lock()
                    .map_err(|_| mlua::Error::external("lock poisoned"))?
                    .apply()
                    .map_err(mlua::Error::external)?;

            let result = lua.create_table()?;
            for (i, f) in files.iter().enumerate() {
                result.set(i + 1, f.to_string_lossy().to_string())?;
            }
            Ok(result)
        })?,
    )?;

    // shadow.worktree.reset() -> discard changes
    worktree_table.set(
        "reset",
        lua.create_function(|lua, ()| {
            let wt: LuaShadowWorktree = lua.named_registry_value("_shadow_worktree")?;
            wt.0.lock()
                .map_err(|_| mlua::Error::external("lock poisoned"))?
                .reset()
                .map_err(mlua::Error::external)?;
            Ok(())
        })?,
    )?;

    // shadow.worktree.modified() -> list of modified file paths
    worktree_table.set(
        "modified",
        lua.create_function(|lua, ()| {
            let wt: LuaShadowWorktree = lua.named_registry_value("_shadow_worktree")?;
            let files: Vec<String> =
                wt.0.lock()
                    .map_err(|_| mlua::Error::external("lock poisoned"))?
                    .modified()
                    .iter()
                    .map(|p| p.to_string_lossy().to_string())
                    .collect();
            Ok(files)
        })?,
    )?;

    // shadow.worktree.enable() -> route edit() through shadow worktree
    // Must call open() first
    worktree_table.set(
        "enable",
        lua.create_function(|lua, ()| {
            let wt: LuaShadowWorktree = lua.named_registry_value("_shadow_worktree")?;
            let path =
                wt.0.lock()
                    .map_err(|_| mlua::Error::external("lock poisoned"))?
                    .path()
                    .to_string_lossy()
                    .to_string();
            lua.set_named_registry_value("_shadow_edit_path", path)?;
            Ok(())
        })?,
    )?;

    // shadow.worktree.disable() -> stop routing edit() through shadow
    worktree_table.set(
        "disable",
        lua.create_function(|lua, ()| {
            lua.set_named_registry_value("_shadow_edit_path", mlua::Value::Nil)?;
            Ok(())
        })?,
    )?;

    // shadow.worktree.enabled() -> check if shadow edit mode is active
    worktree_table.set(
        "enabled",
        lua.create_function(|lua, ()| {
            let path: Option<String> = lua.named_registry_value("_shadow_edit_path").ok();
            Ok(path.is_some())
        })?,
    )?;

    shadow_table.set("worktree", worktree_table)?;

    globals.set("shadow", shadow_table)?;
    Ok(())
}
