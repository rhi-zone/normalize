//! Lua-based workflow runtime using LuaJIT.

use std::path::Path;
#[cfg(feature = "llm")]
use std::path::PathBuf;
use std::process::Command;

use mlua::{FromLua, Lua, Result as LuaResult, Table, Thread, UserData, UserDataMethods, Value};

#[cfg(feature = "llm")]
use super::llm::{parse_agent_response, AgentAction, LlmClient, AGENT_SYSTEM_PROMPT};

/// What the runtime is waiting for from the frontend.
#[derive(Debug, Clone)]
pub enum RuntimeYield {
    /// Waiting for user to enter text.
    Prompt { message: String },
    /// Waiting for user to pick from options.
    Menu { options: Vec<String> },
}

/// State of an interactive workflow.
#[derive(Debug)]
pub enum RuntimeState {
    /// Waiting for input from the frontend.
    Waiting(RuntimeYield),
    /// Finished successfully.
    Done(Option<CommandResult>),
    /// Errored.
    Error(String),
}

/// Lua workflow runtime.
pub struct LuaRuntime {
    lua: Lua,
}

/// Interactive workflow session (coroutine-based).
pub struct WorkflowSession {
    thread: Thread,
}

/// Result of a command execution.
#[derive(Debug, Clone)]
pub struct CommandResult {
    pub output: String,
    pub success: bool,
}

impl UserData for CommandResult {
    fn add_fields<F: mlua::UserDataFields<Self>>(fields: &mut F) {
        fields.add_field_method_get("output", |_, this| Ok(this.output.clone()));
        fields.add_field_method_get("success", |_, this| Ok(this.success));
    }

    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_meta_method("__tostring", |_, this, ()| Ok(this.output.clone()));
    }
}

/// Options for `view` command.
#[derive(Debug, Default)]
struct ViewOpts {
    target: Option<String>,
    depth: Option<i32>,
    deps: bool,
    context: bool,
}

impl FromLua for ViewOpts {
    fn from_lua(value: Value, _lua: &Lua) -> LuaResult<Self> {
        match value {
            Value::Nil => Ok(Self::default()),
            Value::String(s) => Ok(Self {
                target: Some(s.to_str()?.to_string()),
                ..Default::default()
            }),
            Value::Table(t) => Ok(Self {
                target: t.get("target").ok(),
                depth: t.get("depth").ok(),
                deps: t.get("deps").unwrap_or(false),
                context: t.get("context").unwrap_or(false),
            }),
            _ => Err(mlua::Error::FromLuaConversionError {
                from: value.type_name(),
                to: "ViewOpts".to_string(),
                message: None,
            }),
        }
    }
}

/// Options for `analyze` command.
#[derive(Debug, Default)]
struct AnalyzeOpts {
    target: Option<String>,
    health: bool,
    complexity: bool,
}

impl FromLua for AnalyzeOpts {
    fn from_lua(value: Value, _lua: &Lua) -> LuaResult<Self> {
        match value {
            Value::Nil => Ok(Self::default()),
            Value::Table(t) => Ok(Self {
                target: t.get("target").ok(),
                health: t.get("health").unwrap_or(false),
                complexity: t.get("complexity").unwrap_or(false),
            }),
            _ => Err(mlua::Error::FromLuaConversionError {
                from: value.type_name(),
                to: "AnalyzeOpts".to_string(),
                message: None,
            }),
        }
    }
}

/// Options for `grep` command.
#[derive(Debug)]
struct GrepOpts {
    pattern: String,
    path: Option<String>,
    file_type: Option<String>,
}

impl FromLua for GrepOpts {
    fn from_lua(value: Value, _lua: &Lua) -> LuaResult<Self> {
        match value {
            Value::String(s) => Ok(Self {
                pattern: s.to_str()?.to_string(),
                path: None,
                file_type: None,
            }),
            Value::Table(t) => Ok(Self {
                pattern: t.get("pattern")?,
                path: t.get("path").ok(),
                file_type: t.get("type").ok(),
            }),
            _ => Err(mlua::Error::FromLuaConversionError {
                from: value.type_name(),
                to: "GrepOpts".to_string(),
                message: None,
            }),
        }
    }
}

impl LuaRuntime {
    pub fn new(root: &Path) -> LuaResult<Self> {
        let lua = Lua::new();

        {
            let globals = lua.globals();
            let root = root.to_path_buf();

            globals.set("_moss_root", root.to_string_lossy().to_string())?;

            Self::register_commands(&lua, &globals)?;
            Self::register_helpers(&lua, &globals, &root)?;
            Self::register_drivers(&lua, &globals, &root)?;
        }

        Ok(Self { lua })
    }

    pub fn run_file(&self, path: &Path) -> LuaResult<()> {
        let script = std::fs::read_to_string(path)
            .map_err(|e| mlua::Error::external(format!("Failed to read script: {}", e)))?;
        self.run_string(&script)
    }

    pub fn run_string(&self, script: &str) -> LuaResult<()> {
        self.lua.load(script).exec()
    }

    /// Create an interactive workflow session from a script.
    /// The script runs as a coroutine that can yield for user input.
    pub fn create_session(&self, script: &str) -> LuaResult<WorkflowSession> {
        // Wrap script in coroutine.create
        let wrapped = format!(
            r#"return coroutine.create(function()
                {}
            end)"#,
            script
        );
        let thread: Thread = self.lua.load(&wrapped).eval()?;
        Ok(WorkflowSession { thread })
    }
}

impl WorkflowSession {
    /// Start or resume the workflow. Call with None to start, Some(input) to resume.
    pub fn step(&self, input: Option<&str>) -> LuaResult<RuntimeState> {
        use mlua::ThreadStatus;

        match self.thread.status() {
            ThreadStatus::Resumable => {
                // Resume with input (or nothing if starting)
                let result: mlua::MultiValue = if let Some(inp) = input {
                    self.thread.resume(inp)?
                } else {
                    self.thread.resume(())?
                };

                // Check if we yielded or finished
                match self.thread.status() {
                    ThreadStatus::Resumable => {
                        // Yielded - parse what we're waiting for
                        let mut values = result.into_iter();
                        let yield_type = values
                            .next()
                            .and_then(|v| v.as_str().map(|s| s.to_string()));

                        match yield_type.as_deref() {
                            Some("prompt") => {
                                let message = values
                                    .next()
                                    .and_then(|v| v.as_str().map(|s| s.to_string()))
                                    .unwrap_or_default();
                                Ok(RuntimeState::Waiting(RuntimeYield::Prompt { message }))
                            }
                            Some("menu") => {
                                let options = values
                                    .next()
                                    .and_then(|v| {
                                        if let Value::Table(t) = v {
                                            let opts: Vec<String> = t
                                                .sequence_values::<String>()
                                                .filter_map(|r| r.ok())
                                                .collect();
                                            Some(opts)
                                        } else {
                                            None
                                        }
                                    })
                                    .unwrap_or_default();
                                Ok(RuntimeState::Waiting(RuntimeYield::Menu { options }))
                            }
                            _ => Ok(RuntimeState::Error("Unknown yield type".to_string())),
                        }
                    }
                    ThreadStatus::Finished => {
                        // Finished - try to get CommandResult from return value
                        let cmd_result = result.into_iter().next().and_then(|v| {
                            if let Value::UserData(ud) = v {
                                ud.borrow::<CommandResult>().ok().map(|r| r.clone())
                            } else {
                                None
                            }
                        });
                        Ok(RuntimeState::Done(cmd_result))
                    }
                    ThreadStatus::Running => {
                        Ok(RuntimeState::Error("Thread still running".to_string()))
                    }
                    ThreadStatus::Error => Ok(RuntimeState::Error("Thread error".to_string())),
                }
            }
            ThreadStatus::Finished => Ok(RuntimeState::Done(None)),
            ThreadStatus::Running => Ok(RuntimeState::Error("Thread already running".to_string())),
            ThreadStatus::Error => Ok(RuntimeState::Error("Thread in error state".to_string())),
        }
    }
}

impl LuaRuntime {
    fn register_commands(lua: &Lua, globals: &Table) -> LuaResult<()> {
        // TODO: Refactor cmd_* functions to take typed structs, then call directly.
        // For now, convert typed opts to CLI args and use subprocess.

        // view(opts: ViewOpts) -> CommandResult
        globals.set(
            "view",
            lua.create_function(|_, opts: ViewOpts| {
                let mut args = vec!["view".to_string()];
                if let Some(t) = opts.target {
                    args.push(t);
                }
                if opts.deps {
                    args.push("--deps".to_string());
                }
                if opts.context {
                    args.push("--context".to_string());
                }
                if let Some(d) = opts.depth {
                    args.push("--depth".to_string());
                    args.push(d.to_string());
                }
                run_subprocess(&args)
            })?,
        )?;

        // analyze(opts: AnalyzeOpts) -> CommandResult
        globals.set(
            "analyze",
            lua.create_function(|_, opts: AnalyzeOpts| {
                let mut args = vec!["analyze".to_string()];
                if opts.health {
                    args.push("--health".to_string());
                }
                if opts.complexity {
                    args.push("--complexity".to_string());
                }
                if let Some(t) = opts.target {
                    args.push(t);
                }
                run_subprocess(&args)
            })?,
        )?;

        // grep(opts: GrepOpts) -> CommandResult
        globals.set(
            "grep",
            lua.create_function(|_, opts: GrepOpts| {
                let mut args = vec!["grep".to_string(), opts.pattern];
                if let Some(p) = opts.path {
                    args.push(p);
                }
                if let Some(t) = opts.file_type {
                    args.push("--type".to_string());
                    args.push(t);
                }
                run_subprocess(&args)
            })?,
        )?;

        // Simple commands
        macro_rules! simple_command {
            ($name:literal) => {{
                globals.set(
                    $name,
                    lua.create_function(|_, arg: Option<String>| {
                        let mut args = vec![$name.to_string()];
                        if let Some(a) = arg {
                            args.push(a);
                        }
                        run_subprocess(&args)
                    })?,
                )?;
            }};
        }

        simple_command!("edit");
        simple_command!("index");
        simple_command!("lint");
        simple_command!("plans");
        simple_command!("sessions");

        Ok(())
    }

    fn register_helpers(lua: &Lua, globals: &Table, root: &Path) -> LuaResult<()> {
        let root_path = root.to_path_buf();

        // shell(cmd: string) -> CommandResult
        let root_clone = root_path.clone();
        globals.set(
            "shell",
            lua.create_function(move |_, cmd: String| {
                let shell = if cfg!(windows) { "cmd" } else { "sh" };
                let flag = if cfg!(windows) { "/C" } else { "-c" };

                let output = Command::new(shell)
                    .args([flag, &cmd])
                    .current_dir(&root_clone)
                    .output()
                    .map_err(mlua::Error::external)?;

                Ok(CommandResult {
                    output: String::from_utf8_lossy(&output.stdout).to_string(),
                    success: output.status.success(),
                })
            })?,
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
                std::fs::read_to_string(root_clone.join(&path)).map_err(mlua::Error::external)
            })?,
        )?;

        // print(...)
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

        // prompt(message) -> string (yields to frontend)
        // menu(options) -> string (yields to frontend)
        // These are Lua functions because yield must happen from Lua, not Rust
        lua.load(
            r#"
            function prompt(message)
                return coroutine.yield("prompt", message or "")
            end

            function menu(options)
                return coroutine.yield("menu", options)
            end
            "#,
        )
        .exec()?;

        Ok(())
    }

    fn register_drivers(lua: &Lua, globals: &Table, root: &Path) -> LuaResult<()> {
        // auto { model = "...", prompt = "..." } -> CommandResult
        #[cfg(feature = "llm")]
        {
            let root_path = root.to_path_buf();
            globals.set(
                "auto",
                lua.create_function(move |_, config: Table| run_auto_loop(&config, &root_path))?,
            )?;
        }

        #[cfg(not(feature = "llm"))]
        {
            let _ = root; // suppress unused warning
            globals.set(
                "auto",
                lua.create_function(|_, _config: Table| {
                    Err::<CommandResult, _>(mlua::Error::external(
                        "auto{} requires the 'llm' feature. Rebuild with: cargo build --features llm",
                    ))
                })?,
            )?;
        }

        // manual { actions = {...} } - user-driven interactive loop
        // Defined in Lua because it needs to yield for user input
        lua.load(
            r#"
            function manual(config)
                local actions = config.actions
                if not actions then
                    error("manual{} requires actions table")
                end

                -- Build menu options from action names
                local options = {}
                for name, _ in pairs(actions) do
                    table.insert(options, name)
                end
                table.insert(options, "quit")
                table.sort(options)

                -- Main loop
                while true do
                    local choice = menu(options)
                    if choice == "quit" then
                        break
                    end

                    local action = actions[choice]
                    if action then
                        local ok, result = pcall(action)
                        if ok and result then
                            print(result)
                        elseif not ok then
                            print("Error: " .. tostring(result))
                        end
                    end
                end

                return { output = "", success = true }
            end
            "#,
        )
        .exec()?;

        Ok(())
    }
}

/// Run an LLM-driven autonomous loop.
#[cfg(feature = "llm")]
fn run_auto_loop(config: &Table, root: &PathBuf) -> LuaResult<CommandResult> {
    // Parse config
    let model: Option<String> = config.get("model").ok();
    let prompt: String = config
        .get("prompt")
        .unwrap_or_else(|_| "Help me with this codebase.".to_string());
    let max_turns: usize = config.get("max_turns").unwrap_or(10);

    // Extract provider from model (format: "provider/model" or just "provider")
    let (provider, model_name) = if let Some(ref m) = model {
        if let Some((p, n)) = m.split_once('/') {
            (p, Some(n))
        } else {
            (m.as_str(), None)
        }
    } else {
        ("anthropic", None)
    };

    // Create LLM client
    let client = LlmClient::new(provider, model_name).map_err(mlua::Error::external)?;

    // Build conversation
    let mut conversation = format!("Task: {}\n\nCurrent directory: {}", prompt, root.display());
    let mut all_output = String::new();

    for turn in 0..max_turns {
        println!("[auto] Turn {}/{}", turn + 1, max_turns);

        // Get LLM response
        let response = client
            .complete(Some(AGENT_SYSTEM_PROMPT), &conversation)
            .map_err(mlua::Error::external)?;

        println!("{}", response);
        all_output.push_str(&response);
        all_output.push('\n');

        // Parse response
        match parse_agent_response(&response) {
            AgentAction::Command { name, args } => {
                // Execute command
                let mut cmd_args = vec![name.clone()];
                cmd_args.extend(args);

                println!("[auto] Executing: {}", cmd_args.join(" "));

                let result = run_subprocess_in_dir(&cmd_args, root)?;

                // Add result to conversation
                conversation.push_str("\n\nAssistant: ");
                conversation.push_str(&response);
                conversation.push_str("\n\nCommand output:\n");
                conversation.push_str(&result.output);

                if !result.success {
                    conversation.push_str("\n(command failed)");
                }
            }
            AgentAction::Done { message } => {
                println!("[auto] Done: {}", message);
                break;
            }
        }
    }

    Ok(CommandResult {
        output: all_output,
        success: true,
    })
}

/// Run moss subprocess in a specific directory.
#[cfg(feature = "llm")]
fn run_subprocess_in_dir(args: &[String], dir: &Path) -> LuaResult<CommandResult> {
    let exe = std::env::current_exe().map_err(mlua::Error::external)?;
    let output = Command::new(&exe)
        .args(args)
        .current_dir(dir)
        .output()
        .map_err(mlua::Error::external)?;

    Ok(CommandResult {
        output: String::from_utf8_lossy(&output.stdout).to_string(),
        success: output.status.success(),
    })
}

/// Fallback: run moss as subprocess (for commands not yet refactored).
fn run_subprocess(args: &[String]) -> LuaResult<CommandResult> {
    let exe = std::env::current_exe().map_err(mlua::Error::external)?;
    let output = Command::new(&exe)
        .args(args)
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

    #[test]
    fn test_view_opts_from_string() {
        let lua = Lua::new();
        let val = lua.create_string("foo.rs").unwrap();
        let opts = ViewOpts::from_lua(Value::String(val), &lua).unwrap();
        assert_eq!(opts.target, Some("foo.rs".to_string()));
    }

    #[test]
    fn test_view_opts_from_table() {
        let lua = Lua::new();
        lua.load(r#"return { target = "bar.rs", context = true }"#)
            .eval::<Value>()
            .and_then(|v| ViewOpts::from_lua(v, &lua))
            .map(|opts| {
                assert_eq!(opts.target, Some("bar.rs".to_string()));
                assert!(opts.context);
            })
            .unwrap();
    }

    #[test]
    fn test_session_prompt() {
        let runtime = LuaRuntime::new(std::path::Path::new(".")).unwrap();
        let session = runtime
            .create_session(r#"local x = prompt("Enter name: ") return x"#)
            .unwrap();

        // Start - should yield waiting for prompt
        match session.step(None).unwrap() {
            RuntimeState::Waiting(RuntimeYield::Prompt { message }) => {
                assert_eq!(message, "Enter name: ");
            }
            other => panic!("Expected Prompt, got {:?}", other),
        }

        // Resume with input - should finish
        match session.step(Some("Alice")).unwrap() {
            RuntimeState::Done(_) => {}
            other => panic!("Expected Done, got {:?}", other),
        }
    }

    #[test]
    fn test_session_menu() {
        let runtime = LuaRuntime::new(std::path::Path::new(".")).unwrap();
        let session = runtime
            .create_session(r#"local x = menu({"a", "b", "c"}) return x"#)
            .unwrap();

        // Start - should yield waiting for menu
        match session.step(None).unwrap() {
            RuntimeState::Waiting(RuntimeYield::Menu { options }) => {
                assert_eq!(options, vec!["a", "b", "c"]);
            }
            other => panic!("Expected Menu, got {:?}", other),
        }

        // Resume with selection - should finish
        match session.step(Some("b")).unwrap() {
            RuntimeState::Done(_) => {}
            other => panic!("Expected Done, got {:?}", other),
        }
    }

    #[test]
    fn test_session_no_yield() {
        let runtime = LuaRuntime::new(std::path::Path::new(".")).unwrap();
        let session = runtime.create_session(r#"return "done""#).unwrap();

        // Start - should finish immediately
        match session.step(None).unwrap() {
            RuntimeState::Done(_) => {}
            other => panic!("Expected Done, got {:?}", other),
        }
    }

    #[test]
    fn test_manual_driver() {
        let runtime = LuaRuntime::new(std::path::Path::new(".")).unwrap();
        let session = runtime
            .create_session(
                r#"manual{
                    actions = {
                        check = function() return analyze() end,
                        find = function() return grep(prompt("Pattern: ")) end,
                    }
                }"#,
            )
            .unwrap();

        // Start - should show menu with defined actions + quit
        match session.step(None).unwrap() {
            RuntimeState::Waiting(RuntimeYield::Menu { options }) => {
                assert!(options.contains(&"quit".to_string()));
                assert!(options.contains(&"check".to_string()));
                assert!(options.contains(&"find".to_string()));
            }
            other => panic!("Expected Menu, got {:?}", other),
        }

        // Select quit - should finish
        match session.step(Some("quit")).unwrap() {
            RuntimeState::Done(_) => {}
            other => panic!("Expected Done after quit, got {:?}", other),
        }
    }

    #[test]
    fn test_manual_driver_requires_actions() {
        let runtime = LuaRuntime::new(std::path::Path::new(".")).unwrap();
        let session = runtime.create_session(r#"manual{}"#).unwrap();

        // Start - should error because no actions provided
        let result = session.step(None);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("requires actions table"), "Error was: {}", err);
    }
}
