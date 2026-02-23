//! CLI format parsers.
//!
//! # Extensibility
//!
//! Users can register custom formats via [`register()`]:
//!
//! ```ignore
//! use normalize_cli_parser::{CliFormat, CliSpec, register};
//!
//! struct MyFormat;
//!
//! impl CliFormat for MyFormat {
//!     fn name(&self) -> &'static str { "myformat" }
//!     fn detect(&self, help: &str) -> f64 { /* ... */ }
//!     fn parse(&self, help: &str) -> Result<CliSpec, String> { /* ... */ }
//! }
//!
//! // Register before first use
//! register(&MyFormat);
//! ```

mod argparse;
mod clap;
mod click;
mod cobra;
mod commander;
mod yargs;

pub use self::argparse::ArgparseFormat;
pub use self::clap::ClapFormat;
pub use self::click::ClickFormat;
pub use self::cobra::CobraFormat;
pub use self::commander::CommanderFormat;
pub use self::yargs::YargsFormat;

use crate::{CliCommand, CliSpec};
use regex::Regex;
use std::sync::{OnceLock, RwLock};

/// Global registry of CLI format parsers.
static FORMATS: RwLock<Vec<&'static dyn CliFormat>> = RwLock::new(Vec::new());
static INITIALIZED: OnceLock<()> = OnceLock::new();

/// Register a custom CLI format parser.
///
/// Call this before any parsing operations to add custom formats.
/// Built-in formats are registered automatically on first use.
pub fn register(format: &'static dyn CliFormat) {
    FORMATS.write().unwrap().push(format);
}

/// Initialize built-in formats (called automatically on first use).
fn init_builtin() {
    INITIALIZED.get_or_init(|| {
        let mut formats = FORMATS.write().unwrap();
        formats.push(&ClapFormat);
        formats.push(&ArgparseFormat);
        formats.push(&ClickFormat);
        formats.push(&CommanderFormat);
        formats.push(&YargsFormat);
        formats.push(&CobraFormat);
    });
}

/// Trait for CLI help format parsers.
pub trait CliFormat: Send + Sync {
    /// Format name (e.g., "clap", "argparse").
    fn name(&self) -> &'static str;

    /// Confidence score (0.0-1.0) that this format matches the help text.
    fn detect(&self, help_text: &str) -> f64;

    /// Parse help text into a CliSpec.
    fn parse(&self, help_text: &str) -> Result<CliSpec, String>;
}

/// Get a format by name from the global registry.
pub fn get_format(name: &str) -> Option<&'static dyn CliFormat> {
    init_builtin();
    FORMATS
        .read()
        .unwrap()
        .iter()
        .find(|f| f.name() == name)
        .copied()
}

/// Auto-detect format from help text using the global registry.
pub fn detect_format(help_text: &str) -> Option<&'static dyn CliFormat> {
    init_builtin();
    FORMATS
        .read()
        .unwrap()
        .iter()
        .map(|f| (*f, f.detect(help_text)))
        .filter(|(_, score)| *score > 0.5)
        .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
        .map(|(f, _)| f)
}

/// List all available format names from the global registry.
pub fn list_formats() -> Vec<&'static str> {
    init_builtin();
    FORMATS.read().unwrap().iter().map(|f| f.name()).collect()
}

/// Registry of CLI format parsers.
///
/// For most use cases, prefer the global registry via [`register()`],
/// [`get_format()`], [`detect_format()`], and [`list_formats()`].
///
/// Use `FormatRegistry` when you need an isolated registry (e.g., testing).
pub struct FormatRegistry {
    formats: Vec<Box<dyn CliFormat>>,
}

impl FormatRegistry {
    /// Create a new registry with all built-in formats.
    pub fn new() -> Self {
        Self {
            formats: vec![
                Box::new(ClapFormat),
                Box::new(ArgparseFormat),
                Box::new(ClickFormat),
                Box::new(CommanderFormat),
                Box::new(YargsFormat),
                Box::new(CobraFormat),
            ],
        }
    }

    /// Create an empty registry (no built-in formats).
    pub fn empty() -> Self {
        Self { formats: vec![] }
    }

    /// Register a custom format.
    pub fn register(&mut self, format: Box<dyn CliFormat>) {
        self.formats.push(format);
    }

    /// Get a format by name.
    pub fn get(&self, name: &str) -> Option<&dyn CliFormat> {
        self.formats
            .iter()
            .find(|f| f.name() == name)
            .map(|f| f.as_ref())
    }

    /// Auto-detect format from help text.
    pub fn detect(&self, help_text: &str) -> Option<&dyn CliFormat> {
        self.formats
            .iter()
            .map(|f| (f, f.detect(help_text)))
            .filter(|(_, score)| *score > 0.5)
            .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
            .map(|(f, _)| f.as_ref())
    }

    /// List all available format names.
    pub fn list(&self) -> Vec<&'static str> {
        self.formats.iter().map(|f| f.name()).collect()
    }
}

impl Default for FormatRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Detect if a line is a section header (e.g. "Options:", "Commands:").
///
/// Section headers are non-empty, don't start with `-` or space, and end with `:`.
pub(super) fn is_section_header(line: &str) -> bool {
    let trimmed = line.trim();
    !trimmed.is_empty()
        && !trimmed.starts_with('-')
        && !trimmed.starts_with(' ')
        && trimmed.ends_with(':')
}

/// Parse sections of a help output into options and commands.
///
/// Used by format parsers that use "Options:" / "Commands:" section headers.
/// `parse_opt` and `parse_cmd` are the format-specific line parsers.
pub(super) fn parse_option_command_sections<O, C>(
    lines: &[&str],
    parse_opt: impl Fn(&str) -> Option<O>,
    parse_cmd: impl Fn(&str) -> Option<C>,
) -> (Vec<O>, Vec<C>) {
    let mut options = Vec::new();
    let mut commands = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];
        if line == "Options:" {
            i += 1;
            while i < lines.len() && !is_section_header(lines[i]) {
                if let Some(opt) = parse_opt(lines[i]) {
                    options.push(opt);
                }
                i += 1;
            }
        } else if line == "Commands:" {
            i += 1;
            while i < lines.len() && !is_section_header(lines[i]) {
                if let Some(cmd) = parse_cmd(lines[i]) {
                    commands.push(cmd);
                }
                i += 1;
            }
        } else {
            i += 1;
        }
    }
    (options, commands)
}

/// Parse a trimmed help-output line as a subcommand, skipping "help".
///
/// Shared by multiple format parsers that use the same two-or-more-spaces separator pattern.
/// Returns `None` for empty lines, lines starting with `-`, or lines that don't parse.
pub(super) fn parse_command_from_trimmed_line(trimmed: &str) -> Option<CliCommand> {
    if trimmed.is_empty() || trimmed.starts_with('-') {
        return None;
    }
    let re = Regex::new(r"^(\S+)\s{2,}(.*)$").unwrap();
    if let Some(caps) = re.captures(trimmed) {
        let name = caps.get(1)?.as_str().to_string();
        if name == "help" {
            return None;
        }
        Some(CliCommand {
            name,
            description: caps.get(2).map(|m| m.as_str().to_string()),
            aliases: Vec::new(),
            options: Vec::new(),
            subcommands: Vec::new(),
        })
    } else if !trimmed.contains(' ') {
        Some(CliCommand {
            name: trimmed.to_string(),
            description: None,
            aliases: Vec::new(),
            options: Vec::new(),
            subcommands: Vec::new(),
        })
    } else {
        None
    }
}
