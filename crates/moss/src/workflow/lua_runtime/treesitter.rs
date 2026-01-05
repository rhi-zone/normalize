//! Tree-sitter Lua bindings for the workflow runtime.

use std::sync::Arc;

use mlua::{Lua, Result as LuaResult, Table, UserData, UserDataMethods};

use crate::parsers;

/// Wrapper for tree_sitter::Tree with source for Lua.
pub(super) struct LuaTree {
    tree: Arc<tree_sitter::Tree>,
    source: Arc<String>,
}

impl LuaTree {
    pub fn new(tree: tree_sitter::Tree, source: String) -> Self {
        Self {
            tree: Arc::new(tree),
            source: Arc::new(source),
        }
    }
}

impl UserData for LuaTree {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("root", |lua, this, ()| {
            let node = LuaNode {
                tree: this.tree.clone(),
                source: this.source.clone(),
                path: Vec::new(), // Empty path = root
            };
            lua.create_userdata(node)
        });
    }
}

/// Wrapper for tree_sitter::Node for Lua.
/// Stores tree reference and path from root (list of child indices) to reconstruct the node.
struct LuaNode {
    tree: Arc<tree_sitter::Tree>,
    source: Arc<String>,
    /// Path from root as child indices
    path: Vec<usize>,
}

impl LuaNode {
    fn get_node(&self) -> Option<tree_sitter::Node<'_>> {
        let mut node = self.tree.root_node();
        for &index in &self.path {
            node = node.child(index)?;
        }
        Some(node)
    }

    fn wrap_child(&self, child_index: usize) -> Self {
        let mut path = self.path.clone();
        path.push(child_index);
        LuaNode {
            tree: self.tree.clone(),
            source: self.source.clone(),
            path,
        }
    }
}

impl UserData for LuaNode {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        // node:kind() -> string
        methods.add_method("kind", |_, this, ()| {
            let node = this
                .get_node()
                .ok_or_else(|| mlua::Error::external("Node not found"))?;
            Ok(node.kind().to_string())
        });

        // node:text() -> string
        methods.add_method("text", |_, this, ()| {
            let node = this
                .get_node()
                .ok_or_else(|| mlua::Error::external("Node not found"))?;
            let start = node.start_byte();
            let end = node.end_byte();
            Ok(this.source[start..end].to_string())
        });

        // node:start_row() -> number (1-indexed for Lua)
        methods.add_method("start_row", |_, this, ()| {
            let node = this
                .get_node()
                .ok_or_else(|| mlua::Error::external("Node not found"))?;
            Ok(node.start_position().row + 1)
        });

        // node:end_row() -> number (1-indexed)
        methods.add_method("end_row", |_, this, ()| {
            let node = this
                .get_node()
                .ok_or_else(|| mlua::Error::external("Node not found"))?;
            Ok(node.end_position().row + 1)
        });

        // node:child_count() -> number
        methods.add_method("child_count", |_, this, ()| {
            let node = this
                .get_node()
                .ok_or_else(|| mlua::Error::external("Node not found"))?;
            Ok(node.child_count())
        });

        // node:child(index) -> LuaNode (1-indexed for Lua)
        methods.add_method("child", |lua, this, index: usize| {
            let node = this
                .get_node()
                .ok_or_else(|| mlua::Error::external("Node not found"))?;
            let child_index = index.saturating_sub(1);
            let _child = node
                .child(child_index)
                .ok_or_else(|| mlua::Error::external("Child not found"))?;
            lua.create_userdata(this.wrap_child(child_index))
        });

        // node:children() -> table of LuaNodes
        methods.add_method("children", |lua, this, ()| {
            let node = this
                .get_node()
                .ok_or_else(|| mlua::Error::external("Node not found"))?;
            let result = lua.create_table()?;
            for i in 0..node.child_count() {
                result.set(i + 1, lua.create_userdata(this.wrap_child(i))?)?;
            }
            Ok(result)
        });

        // node:named_children() -> table of LuaNodes (excludes anonymous nodes)
        methods.add_method("named_children", |lua, this, ()| {
            let node = this
                .get_node()
                .ok_or_else(|| mlua::Error::external("Node not found"))?;
            let result = lua.create_table()?;
            let mut lua_index = 1;
            for i in 0..node.child_count() {
                if let Some(child) = node.child(i) {
                    if child.is_named() {
                        result.set(lua_index, lua.create_userdata(this.wrap_child(i))?)?;
                        lua_index += 1;
                    }
                }
            }
            Ok(result)
        });

        // node:child_by_field(name) -> LuaNode or nil
        methods.add_method("child_by_field", |lua, this, name: String| {
            let node = this
                .get_node()
                .ok_or_else(|| mlua::Error::external("Node not found"))?;
            // Find the index of the child with this field name
            for i in 0..node.child_count() {
                if node.field_name_for_child(i as u32) == Some(&name) {
                    return Ok(Some(lua.create_userdata(this.wrap_child(i))?));
                }
            }
            Ok(None)
        });

        // node:is_named() -> boolean
        methods.add_method("is_named", |_, this, ()| {
            let node = this
                .get_node()
                .ok_or_else(|| mlua::Error::external("Node not found"))?;
            Ok(node.is_named())
        });

        // node:parent() -> LuaNode or nil
        methods.add_method("parent", |lua, this, ()| {
            if this.path.is_empty() {
                return Ok(None); // Root has no parent
            }
            let mut parent_path = this.path.clone();
            parent_path.pop();
            let parent = LuaNode {
                tree: this.tree.clone(),
                source: this.source.clone(),
                path: parent_path,
            };
            Ok(Some(lua.create_userdata(parent)?))
        });

        // node:next_sibling() -> LuaNode or nil
        methods.add_method("next_sibling", |lua, this, ()| {
            if this.path.is_empty() {
                return Ok(None); // Root has no siblings
            }
            let mut sibling_path = this.path.clone();
            let last_index = sibling_path.pop().unwrap();

            // Check if next sibling exists
            let mut parent = this.tree.root_node();
            for &index in &sibling_path {
                parent = parent
                    .child(index)
                    .ok_or_else(|| mlua::Error::external("Path invalid"))?;
            }
            if last_index + 1 >= parent.child_count() {
                return Ok(None);
            }

            sibling_path.push(last_index + 1);
            let sibling = LuaNode {
                tree: this.tree.clone(),
                source: this.source.clone(),
                path: sibling_path,
            };
            Ok(Some(lua.create_userdata(sibling)?))
        });

        // node:prev_sibling() -> LuaNode or nil
        methods.add_method("prev_sibling", |lua, this, ()| {
            if this.path.is_empty() {
                return Ok(None); // Root has no siblings
            }
            let mut sibling_path = this.path.clone();
            let last_index = sibling_path.pop().unwrap();

            if last_index == 0 {
                return Ok(None);
            }

            sibling_path.push(last_index - 1);
            let sibling = LuaNode {
                tree: this.tree.clone(),
                source: this.source.clone(),
                path: sibling_path,
            };
            Ok(Some(lua.create_userdata(sibling)?))
        });
    }
}

/// Register tree-sitter bindings in Lua globals.
pub(super) fn register(lua: &Lua, globals: &Table) -> LuaResult<()> {
    let ts_table = lua.create_table()?;

    // ts.parse(source, grammar) -> LuaTree
    ts_table.set(
        "parse",
        lua.create_function(|lua, (source, grammar): (String, String)| {
            let tree = parsers::parse_with_grammar(&grammar, &source).ok_or_else(|| {
                mlua::Error::external(format!("Failed to parse with grammar '{}'", grammar))
            })?;

            let lua_tree = LuaTree::new(tree, source);
            lua.create_userdata(lua_tree)
        })?,
    )?;

    globals.set("ts", ts_table)?;
    Ok(())
}
