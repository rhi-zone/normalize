//! TypeScript compiler adapter - type checker.
//!
//! TypeScript's tsc is the official type checker for TypeScript projects.
//! https://www.typescriptlang.org/

use crate::{Diagnostic, Tool, ToolCategory, ToolError, ToolInfo, ToolResult};
use std::path::Path;
use std::process::Command;

fn tsc_command() -> Option<(String, Vec<String>)> {
    // tsc binary comes from the "typescript" package
    crate::tools::find_js_tool("tsc", Some("typescript"))
}

/// TypeScript compiler (tsc) type checker adapter.
pub struct Tsc;

const TSC_INFO: ToolInfo = ToolInfo {
    name: "tsc",
    category: ToolCategory::TypeChecker,
    extensions: &["ts", "tsx", "mts", "cts"],
    check_cmd: &["tsc", "--version"],
    website: "https://www.typescriptlang.org/",
};

impl Tsc {
    pub fn new() -> Self {
        Self
    }
}

impl Default for Tsc {
    fn default() -> Self {
        Self
    }
}

impl Tool for Tsc {
    fn info(&self) -> &ToolInfo {
        &TSC_INFO
    }

    fn is_available(&self) -> bool {
        tsc_command().is_some()
    }

    fn version(&self) -> Option<String> {
        let (cmd, base_args) = tsc_command()?;
        let mut command = Command::new(cmd);
        command.args(&base_args).arg("--version");
        command
            .output()
            .ok()
            .filter(|o| o.status.success())
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .map(|s| s.trim().to_string())
    }

    fn detect(&self, root: &Path) -> f32 {
        if crate::tools::has_config_file(root, &["tsconfig.json"]) {
            1.0
        } else {
            0.0
        }
    }

    fn run(&self, paths: &[&Path], root: &Path) -> Result<ToolResult, ToolError> {
        let (cmd_name, base_args) =
            tsc_command().ok_or_else(|| ToolError::NotAvailable("tsc not found".to_string()))?;

        // tsc --noEmit for type checking only
        // Use --pretty false for machine-readable output
        let mut cmd = Command::new(cmd_name);
        cmd.args(&base_args);
        cmd.arg("--noEmit").arg("--pretty").arg("false");

        // If specific paths provided, we can't easily pass them to tsc
        // tsc works on the whole project based on tsconfig.json
        if !paths.is_empty() {
            // Add files explicitly if no tsconfig
            for path in paths {
                if let Some(p) = path.to_str() {
                    cmd.arg(p);
                }
            }
        }

        let output = cmd.current_dir(root).output()?;

        // tsc outputs to stderr for errors
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let combined = format!("{}{}", stdout, stderr);

        if combined.trim().is_empty() || output.status.success() {
            return Ok(ToolResult::success("tsc", vec![]));
        }

        // Parse tsc output: file(line,col): error TSxxxx: message
        let diagnostics = parse_tsc_output(&combined);

        Ok(ToolResult::success("tsc", diagnostics))
    }
}

/// Parse TypeScript compiler output.
///
/// Format: `file.ts(10,5): error TS2322: Type 'string' is not assignable to type 'number'.`
fn parse_tsc_output(output: &str) -> Vec<Diagnostic> {
    super::parse_ts_compiler_output(output, "tsc")
}
