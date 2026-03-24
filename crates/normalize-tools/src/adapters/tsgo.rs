//! Tsgo adapter - native TypeScript type checker.
//!
//! Tsgo is the native TypeScript implementation from Microsoft, written in Go.
//! ~10x faster than tsc for type checking. Will become TypeScript 7.
//! https://github.com/microsoft/typescript-go

use crate::{Diagnostic, Tool, ToolCategory, ToolError, ToolInfo, ToolResult};
use std::path::Path;
use std::process::Command;

fn tsgo_command(root: &std::path::Path) -> Option<crate::tools::ToolInvocation> {
    // @typescript/native-preview provides the tsgo binary
    crate::tools::find_js_tool("tsgo", Some("@typescript/native-preview"), root)
}

/// Tsgo native TypeScript type checker adapter.
pub struct Tsgo;

const TSGO_INFO: ToolInfo = ToolInfo {
    name: "tsgo",
    category: ToolCategory::TypeChecker,
    extensions: &["ts", "tsx", "mts", "cts"],
    check_cmd: &["tsgo", "--version"],
    website: "https://github.com/microsoft/typescript-go",
};

impl Tsgo {
    pub fn new() -> Self {
        Self
    }
}

impl Default for Tsgo {
    fn default() -> Self {
        Self
    }
}

impl Tool for Tsgo {
    fn info(&self) -> &ToolInfo {
        &TSGO_INFO
    }

    fn is_available(&self) -> bool {
        tsgo_command(std::path::Path::new(".")).is_some()
    }

    fn version(&self) -> Option<String> {
        let inv = tsgo_command(std::path::Path::new("."))?;
        let mut command = Command::new(&inv.command);
        command.args(&inv.args).arg("--version");
        command
            .output()
            .ok()
            .filter(|o| o.status.success())
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .map(|s| s.trim().to_string())
    }

    fn detect(&self, root: &Path) -> f32 {
        // Prefer tsgo over tsc when tsconfig exists (tsgo is faster)
        if crate::tools::has_config_file(root, &["tsconfig.json"]) {
            1.0
        } else {
            0.0
        }
    }

    fn run(&self, paths: &[&Path], root: &Path) -> Result<ToolResult, ToolError> {
        let inv = tsgo_command(root)
            .ok_or_else(|| ToolError::NotAvailable("tsgo not found".to_string()))?;

        // tsgo uses similar flags to tsc
        let mut cmd = Command::new(&inv.command);
        cmd.args(&inv.args);
        cmd.arg("--noEmit").arg("--pretty").arg("false");

        // If specific paths provided, pass them
        if !paths.is_empty() {
            for path in paths {
                if let Some(p) = path.to_str() {
                    cmd.arg(p);
                }
            }
        }

        let output = cmd.current_dir(root).output()?;

        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let combined = format!("{}{}", stdout, stderr);

        if combined.trim().is_empty() || output.status.success() {
            return Ok(ToolResult::success("tsgo", vec![]));
        }

        // Parse output - same format as tsc
        let diagnostics = parse_tsgo_output(&combined);

        Ok(ToolResult::success("tsgo", diagnostics))
    }
}

/// Parse tsgo output (same format as tsc).
///
/// Format: `file.ts(10,5): error TS2322: Type 'string' is not assignable to type 'number'.`
fn parse_tsgo_output(output: &str) -> Vec<Diagnostic> {
    super::parse_ts_compiler_output(output, "tsgo")
}
