//! Lua-based workflow runtime using LuaJIT.

use std::path::Path;
use std::process::Command;

use mlua::{Lua, Result as LuaResult, Table, UserData, UserDataMethods, Value};

/// Lua workflow runtime.
pub struct LuaRuntime {
    lua: Lua,
}

/// Result of a command execution.
#[derive(Clone)]
struct CommandResult {
    output: String,
    success: bool,
}

impl UserData for CommandResult {
    fn add_fields<F: mlua::UserDataFields<Self>>(fields: &mut F) {
        fields.add_field_method_get("output", |_, this| Ok(this.output.clone()));
        fields.add_field_method_get("success", |_, this| Ok(this.success));
    }

    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        // Allow using result as string directly
        methods.add_meta_method("__tostring", |_, this, ()| Ok(this.output.clone()));
    }
}

impl LuaRuntime {
    /// Create a new Lua runtime for workflows.
    pub fn new(root: &Path) -> LuaResult<Self> {
        let lua = Lua::new();

        {
            let globals = lua.globals();
            let root = root.to_path_buf();

            globals.set("_moss_root", root.to_string_lossy().to_string())?;

            Self::register_moss_commands(&lua, &globals)?;
            Self::register_helpers(&lua, &globals, &root)?;
            Self::register_drivers(&lua, &globals)?;
        }

        Ok(Self { lua })
    }

    /// Run a Lua workflow script.
    pub fn run_file(&self, path: &Path) -> LuaResult<()> {
        let script = std::fs::read_to_string(path)
            .map_err(|e| mlua::Error::external(format!("Failed to read script: {}", e)))?;
        self.run_string(&script)
    }

    /// Run a Lua workflow from a string.
    pub fn run_string(&self, script: &str) -> LuaResult<()> {
        self.lua.load(script).exec()
    }

    /// Register moss commands as Lua functions.
    fn register_moss_commands(lua: &Lua, globals: &Table) -> LuaResult<()> {
        // Generic moss command executor
        let moss_fn = lua.create_function(|_, args: mlua::Variadic<String>| {
            let args: Vec<String> = args.into_iter().collect();
            run_moss_command(&args)
        })?;
        globals.set("moss", moss_fn)?;

        // Convenience wrappers for common commands
        macro_rules! moss_command {
            ($name:literal) => {{
                let func = lua.create_function(|_, args: mlua::Variadic<String>| {
                    let mut full_args = vec![$name.to_string()];
                    full_args.extend(args.into_iter());
                    run_moss_command(&full_args)
                })?;
                globals.set($name, func)?;
            }};
        }

        moss_command!("view");
        moss_command!("edit");
        moss_command!("analyze");
        moss_command!("grep");
        moss_command!("index");
        moss_command!("lint");
        moss_command!("plans");
        moss_command!("sessions");

        Ok(())
    }

    /// Register helper functions.
    fn register_helpers(lua: &Lua, globals: &Table, root: &Path) -> LuaResult<()> {
        let root_path = root.to_path_buf();

        // shell(cmd: string) -> CommandResult
        let root_clone = root_path.clone();
        globals.set(
            "shell",
            lua.create_function(move |_, cmd: String| run_shell_command(&cmd, &root_clone))?,
        )?;

        // is_dirty() -> boolean
        let root_clone = root_path.clone();
        globals.set(
            "is_dirty",
            lua.create_function(move |_, ()| {
                let output = Command::new("git")
                    .args(["status", "--porcelain"])
                    .current_dir(&root_clone)
                    .output()
                    .map_err(mlua::Error::external)?;
                Ok(!output.stdout.is_empty())
            })?,
        )?;

        // tests_pass() -> boolean
        let root_clone = root_path.clone();
        globals.set(
            "tests_pass",
            lua.create_function(move |_, ()| {
                let status = Command::new("cargo")
                    .args(["test", "--quiet"])
                    .current_dir(&root_clone)
                    .status()
                    .map_err(mlua::Error::external)?;
                Ok(status.success())
            })?,
        )?;

        // file_exists(path: string) -> boolean
        let root_clone = root_path.clone();
        globals.set(
            "file_exists",
            lua.create_function(move |_, path: String| Ok(root_clone.join(&path).exists()))?,
        )?;

        // read_file(path: string) -> string
        let root_clone = root_path.clone();
        globals.set(
            "read_file",
            lua.create_function(move |_, path: String| {
                let full_path = root_clone.join(&path);
                std::fs::read_to_string(&full_path).map_err(mlua::Error::external)
            })?,
        )?;

        // print(...) - format values nicely
        globals.set(
            "print",
            lua.create_function(|lua, args: mlua::Variadic<Value>| {
                let parts: Vec<String> = args
                    .iter()
                    .map(|v| match v {
                        Value::String(s) => s.to_str().map(|s| s.to_string()).unwrap_or_default(),
                        Value::Integer(i) => i.to_string(),
                        Value::Number(n) => n.to_string(),
                        Value::Boolean(b) => b.to_string(),
                        Value::Nil => "nil".to_string(),
                        Value::UserData(ud) => {
                            // Call __tostring metamethod if available
                            let tostring: mlua::Function = lua.globals().get("tostring").unwrap();
                            tostring
                                .call::<String>(ud.clone())
                                .unwrap_or_else(|_| format!("{:?}", v))
                        }
                        other => format!("{:?}", other),
                    })
                    .collect();
                println!("{}", parts.join("\t"));
                Ok(())
            })?,
        )?;

        Ok(())
    }

    /// Register driver functions for agent loops.
    fn register_drivers(lua: &Lua, globals: &Table) -> LuaResult<()> {
        // auto { tools = {...}, model = "..." } - LLM-driven loop
        globals.set(
            "auto",
            lua.create_function(|_, config: Table| {
                let tools: Option<Vec<String>> = config.get("tools")?;
                let model: Option<String> = config.get("model")?;

                println!("[auto] LLM-driven loop");
                if let Some(m) = model {
                    println!("  model: {}", m);
                }
                if let Some(t) = tools {
                    println!("  tools: {}", t.join(", "));
                }
                println!("  (not yet implemented)");

                Ok(())
            })?,
        )?;

        // manual { tools = {...} } - user-driven loop
        globals.set(
            "manual",
            lua.create_function(|_, config: Table| {
                let tools: Option<Vec<String>> = config.get("tools")?;

                println!("[manual] User-driven loop");
                if let Some(t) = tools {
                    println!("  tools: {}", t.join(", "));
                }
                println!("  (not yet implemented)");

                Ok(())
            })?,
        )?;

        Ok(())
    }
}

/// Run a moss command and return result.
fn run_moss_command(args: &[String]) -> LuaResult<CommandResult> {
    let current_exe = std::env::current_exe().map_err(mlua::Error::external)?;

    let output = Command::new(&current_exe)
        .args(args)
        .output()
        .map_err(mlua::Error::external)?;

    Ok(CommandResult {
        output: String::from_utf8_lossy(&output.stdout).to_string(),
        success: output.status.success(),
    })
}

/// Run a shell command and return result.
fn run_shell_command(cmd: &str, root: &Path) -> LuaResult<CommandResult> {
    let shell = if cfg!(windows) { "cmd" } else { "sh" };
    let flag = if cfg!(windows) { "/C" } else { "-c" };

    let output = Command::new(shell)
        .args([flag, cmd])
        .current_dir(root)
        .output()
        .map_err(mlua::Error::external)?;

    Ok(CommandResult {
        output: String::from_utf8_lossy(&output.stdout).to_string(),
        success: output.status.success(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn test_lua_runtime_creation() {
        let root = env::current_dir().unwrap();
        let runtime = LuaRuntime::new(&root).unwrap();
        runtime.run_string("x = 1 + 1").unwrap();
    }

    #[test]
    fn test_lua_helpers() {
        let root = env::current_dir().unwrap();
        let runtime = LuaRuntime::new(&root).unwrap();
        runtime
            .run_string(r#"assert(file_exists("Cargo.toml"), "Cargo.toml should exist")"#)
            .unwrap();
    }

    #[test]
    fn test_command_result() {
        let root = env::current_dir().unwrap();
        let runtime = LuaRuntime::new(&root).unwrap();
        runtime
            .run_string(
                r#"
                local result = shell("echo hello")
                assert(result.success, "command should succeed")
                assert(result.output:match("hello"), "output should contain hello")
            "#,
            )
            .unwrap();
    }
}
