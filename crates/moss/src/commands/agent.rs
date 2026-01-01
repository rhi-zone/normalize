//! Agent command - autonomous task execution.

use crate::workflow::LuaRuntime;
use std::path::Path;

/// Run the agent with a task prompt.
pub fn run(prompt: Option<&str>, max_turns: Option<usize>, root: Option<&Path>) -> i32 {
    let root = match root {
        Some(r) => r.to_path_buf(),
        None => match std::env::current_dir() {
            Ok(d) => d,
            Err(e) => {
                eprintln!("Failed to get current directory: {}", e);
                return 1;
            }
        },
    };

    let runtime = match LuaRuntime::new(&root) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Failed to create Lua runtime: {}", e);
            return 1;
        }
    };

    let prompt_str = prompt.unwrap_or("Help with this codebase");
    let max_turns_str = max_turns.map(|n| n.to_string()).unwrap_or_default();

    let lua_code = format!(
        r#"
        local agent = require("agent")
        local result = agent.run {{
            prompt = {prompt:?},
            max_turns = {max_turns}
        }}
        if not result.success then
            os.exit(1)
        end
        "#,
        prompt = prompt_str,
        max_turns = if max_turns.is_some() {
            max_turns_str.as_str()
        } else {
            "nil"
        }
    );

    match runtime.run_string(&lua_code) {
        Ok(_) => 0,
        Err(e) => {
            eprintln!("Agent error: {}", e);
            1
        }
    }
}
