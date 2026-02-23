//! Tool adapters.
//!
//! Each adapter wraps an external tool and provides:
//! - Availability detection
//! - Project relevance detection
//! - Output parsing to diagnostics

// Python tools
#[cfg(feature = "tool-mypy")]
mod mypy;
#[cfg(feature = "tool-pyright")]
mod pyright;
#[cfg(feature = "tool-ruff")]
mod ruff;

// JavaScript/TypeScript tools
#[cfg(feature = "tool-biome")]
mod biome;
#[cfg(feature = "tool-deno")]
mod deno;
#[cfg(feature = "tool-eslint")]
mod eslint;
#[cfg(feature = "tool-oxfmt")]
mod oxfmt;
#[cfg(feature = "tool-oxlint")]
mod oxlint;
#[cfg(feature = "tool-prettier")]
mod prettier;
#[cfg(feature = "tool-tsc")]
mod tsc;
#[cfg(feature = "tool-tsgo")]
mod tsgo;

// Rust tools
#[cfg(feature = "tool-clippy")]
mod clippy;
#[cfg(feature = "tool-rustfmt")]
mod rustfmt;

// Go tools
#[cfg(any(feature = "tool-gofmt", feature = "tool-govet"))]
mod gofmt;

// Re-exports
#[cfg(feature = "tool-biome")]
pub use biome::{BiomeFormat, BiomeLint};
#[cfg(feature = "tool-clippy")]
pub use clippy::Clippy;
#[cfg(feature = "tool-deno")]
pub use deno::Deno;
#[cfg(feature = "tool-eslint")]
pub use eslint::Eslint;
#[cfg(feature = "tool-gofmt")]
pub use gofmt::Gofmt;
#[cfg(feature = "tool-govet")]
pub use gofmt::Govet;
#[cfg(feature = "tool-mypy")]
pub use mypy::Mypy;
#[cfg(feature = "tool-oxfmt")]
pub use oxfmt::Oxfmt;
#[cfg(feature = "tool-oxlint")]
pub use oxlint::Oxlint;
#[cfg(feature = "tool-prettier")]
pub use prettier::Prettier;
#[cfg(feature = "tool-pyright")]
pub use pyright::Pyright;
#[cfg(feature = "tool-ruff")]
pub use ruff::Ruff;
#[cfg(feature = "tool-rustfmt")]
pub use rustfmt::Rustfmt;
#[cfg(feature = "tool-tsc")]
pub use tsc::Tsc;
#[cfg(feature = "tool-tsgo")]
pub use tsgo::Tsgo;

use crate::Tool;

/// Parse TypeScript compiler output (shared between tsc and tsgo).
///
/// Format: `file.ts(10,5): error TS2322: Type 'string' is not assignable to type 'number'.`
#[cfg(any(feature = "tool-tsc", feature = "tool-tsgo"))]
pub(super) fn parse_ts_compiler_output(output: &str, tool_name: &str) -> Vec<crate::Diagnostic> {
    use crate::{Diagnostic, DiagnosticSeverity, Location};

    let mut diagnostics = Vec::new();
    for line in output.lines() {
        if let Some((loc_part, rest)) = line.split_once("): ")
            && let Some((file, pos)) = loc_part.rsplit_once('(')
        {
            let parts: Vec<&str> = pos.split(',').collect();
            if parts.len() >= 2 {
                let line_num = parts[0].parse().unwrap_or(1);
                let col_num = parts[1].parse().unwrap_or(1);
                let (severity, code, message) = if let Some((sev_code, msg)) = rest.split_once(": ")
                {
                    let (sev, code) = sev_code.split_once(' ').unwrap_or((sev_code, ""));
                    let severity = match sev {
                        "error" => DiagnosticSeverity::Error,
                        "warning" => DiagnosticSeverity::Warning,
                        _ => DiagnosticSeverity::Error,
                    };
                    (severity, code.to_string(), msg.to_string())
                } else {
                    (
                        DiagnosticSeverity::Error,
                        "unknown".to_string(),
                        rest.to_string(),
                    )
                };
                diagnostics.push(Diagnostic {
                    tool: tool_name.to_string(),
                    rule_id: code,
                    message,
                    severity,
                    location: Location {
                        file: file.to_string().into(),
                        line: line_num,
                        column: col_num,
                        end_line: None,
                        end_column: None,
                    },
                    fix: None,
                    help_url: None,
                });
            }
        }
    }
    diagnostics
}

/// Create a registry with all built-in adapters.
#[allow(clippy::vec_init_then_push)]
pub fn all_adapters() -> Vec<Box<dyn Tool>> {
    let mut adapters: Vec<Box<dyn Tool>> = Vec::new();

    // Python
    #[cfg(feature = "tool-ruff")]
    adapters.push(Box::new(Ruff::new()));
    #[cfg(feature = "tool-mypy")]
    adapters.push(Box::new(Mypy::new()));
    #[cfg(feature = "tool-pyright")]
    adapters.push(Box::new(Pyright::new()));

    // JavaScript/TypeScript
    #[cfg(feature = "tool-oxlint")]
    adapters.push(Box::new(Oxlint::new()));
    #[cfg(feature = "tool-oxfmt")]
    adapters.push(Box::new(Oxfmt::new()));
    #[cfg(feature = "tool-eslint")]
    adapters.push(Box::new(Eslint::new()));
    #[cfg(feature = "tool-biome")]
    {
        adapters.push(Box::new(BiomeLint::new()));
        adapters.push(Box::new(BiomeFormat::new()));
    }
    #[cfg(feature = "tool-prettier")]
    adapters.push(Box::new(Prettier::new()));
    #[cfg(feature = "tool-tsgo")]
    adapters.push(Box::new(Tsgo::new()));
    #[cfg(feature = "tool-tsc")]
    adapters.push(Box::new(Tsc::new()));
    #[cfg(feature = "tool-deno")]
    adapters.push(Box::new(Deno::new()));

    // Rust
    #[cfg(feature = "tool-clippy")]
    adapters.push(Box::new(Clippy::new()));
    #[cfg(feature = "tool-rustfmt")]
    adapters.push(Box::new(Rustfmt::new()));

    // Go
    #[cfg(feature = "tool-gofmt")]
    adapters.push(Box::new(Gofmt::new()));
    #[cfg(feature = "tool-govet")]
    adapters.push(Box::new(Govet::new()));

    adapters
}
