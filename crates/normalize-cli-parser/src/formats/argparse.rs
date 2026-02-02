//! Parser for Python argparse-style --help output.
//!
//! Argparse format characteristics:
//! - First line: `usage: <name> ...` (lowercase "usage")
//! - Description follows after blank line
//! - `positional arguments:` section for subcommands
//! - `options:` section (lowercase) with `-s, --long VALUE  Description`

use super::CliFormat;
use crate::{CliCommand, CliOption, CliSpec};
use regex::Regex;

/// Parser for Python argparse-style CLI help output.
pub struct ArgparseFormat;

impl CliFormat for ArgparseFormat {
    fn name(&self) -> &'static str {
        "argparse"
    }

    fn detect(&self, help_text: &str) -> f64 {
        let mut score: f64 = 0.0;

        // Check for "usage:" (lowercase, argparse style)
        if help_text.starts_with("usage:") {
            score += 0.4;
        }

        // Check for "positional arguments:" section
        if help_text.contains("positional arguments:") {
            score += 0.3;
        }

        // Check for "options:" section (lowercase)
        if help_text.contains("\noptions:\n") || help_text.contains("\noptions:\r\n") {
            score += 0.2;
        }

        // Check for "optional arguments:" (older argparse)
        if help_text.contains("optional arguments:") {
            score += 0.2;
        }

        // Negative: check for "Options:" (uppercase, more likely clap)
        if help_text.contains("\nOptions:\n") {
            score -= 0.3;
        }

        score.clamp(0.0, 1.0)
    }

    fn parse(&self, help_text: &str) -> Result<CliSpec, String> {
        let mut spec = CliSpec::default();
        let lines: Vec<&str> = help_text.lines().collect();

        if lines.is_empty() {
            return Err("Empty help text".to_string());
        }

        let mut i = 0;

        // Parse first line: "usage: <name> ..."
        if let Some(first_line) = lines.first() {
            if let Some(usage) = first_line.strip_prefix("usage:") {
                let usage = usage.trim();
                spec.usage = Some(usage.to_string());
                // Extract name from usage
                if let Some(name) = usage.split_whitespace().next() {
                    spec.name = Some(name.to_string());
                }
            }
            i += 1;
        }

        // Parse description (lines until a section header)
        let mut description_lines = Vec::new();
        while i < lines.len() {
            let line = lines[i];
            if is_section_header(line) {
                break;
            }
            if !line.is_empty() {
                description_lines.push(line.trim());
            }
            i += 1;
        }
        if !description_lines.is_empty() {
            spec.description = Some(description_lines.join(" "));
        }

        // Parse sections
        while i < lines.len() {
            let line = lines[i];

            if line == "positional arguments:" {
                i += 1;
                while i < lines.len() && !is_section_header(lines[i]) {
                    if let Some(cmd) = parse_positional_line(lines[i]) {
                        spec.commands.push(cmd);
                    }
                    i += 1;
                }
            } else if line == "options:" || line == "optional arguments:" {
                i += 1;
                while i < lines.len() && !is_section_header(lines[i]) {
                    if let Some(opt) = parse_option_line(lines[i]) {
                        spec.options.push(opt);
                    }
                    i += 1;
                }
            } else {
                i += 1;
            }
        }

        Ok(spec)
    }
}

/// Check if a line is a section header.
fn is_section_header(line: &str) -> bool {
    let trimmed = line.trim();
    !trimmed.is_empty()
        && !trimmed.starts_with('-')
        && !trimmed.starts_with(' ')
        && trimmed.ends_with(':')
}

/// Parse a positional argument line that represents a subcommand.
/// Format: "  {cmd1,cmd2,cmd3}  Description" or "    cmd  Description"
fn parse_positional_line(line: &str) -> Option<CliCommand> {
    let trimmed = line.trim();
    if trimmed.is_empty() || trimmed.starts_with('-') {
        return None;
    }

    // Check for {cmd1,cmd2,...} format (subcommand choices shown together)
    if trimmed.starts_with('{') {
        return None; // Skip the choices line, individual commands follow
    }

    // Parse "  cmd  Description" format
    let re = Regex::new(r"^(\S+)\s{2,}(.*)$").unwrap();
    if let Some(caps) = re.captures(trimmed) {
        let name = caps.get(1)?.as_str().to_string();
        let description = caps.get(2).map(|m| m.as_str().to_string());

        // Skip "help" command
        if name == "help" {
            return None;
        }

        Some(CliCommand {
            name,
            description,
            aliases: Vec::new(),
            options: Vec::new(),
            subcommands: Vec::new(),
        })
    } else if !trimmed.contains(' ') {
        // Just a command name
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

/// Parse an option line.
/// Formats:
/// - "-h, --help  Description"
/// - "-v, --verbose  Description"
/// - "-c, --config FILE  Description"
/// - "--long VALUE  Description"
fn parse_option_line(line: &str) -> Option<CliOption> {
    let trimmed = line.trim();
    if trimmed.is_empty() || !trimmed.starts_with('-') {
        return None;
    }

    let mut opt = CliOption {
        short: None,
        long: None,
        value: None,
        description: None,
        default: None,
        required: false,
        env: None,
    };

    // Match patterns like "-h, --help", "-c, --config FILE", etc.
    // Group 1: short flag (-x)
    // Group 2: long flag (--xxx)
    // Group 3: value (FILE, PORT, etc. - no angle brackets in argparse)
    // Group 4: description
    let re = Regex::new(r"^(-\w)?(?:,\s*)?(--[\w-]+)?(?:\s+([A-Z_]+))?\s{2,}(.*)$").unwrap();

    if let Some(caps) = re.captures(trimmed) {
        opt.short = caps.get(1).map(|m| m.as_str().to_string());
        opt.long = caps.get(2).map(|m| m.as_str().to_string());
        opt.value = caps.get(3).map(|m| format!("<{}>", m.as_str()));
        opt.description = caps.get(4).map(|m| m.as_str().to_string());

        // Check for default value in description: "(default: X)"
        if let Some(ref desc) = opt.description
            && let Some(start) = desc.find("(default:")
            && let Some(end) = desc[start..].find(')')
        {
            let default = desc[start + 9..start + end].trim().to_string();
            opt.default = Some(default);
        }

        // Skip help as it's meta
        if opt.long == Some("--help".to_string()) {
            return None;
        }

        Some(opt)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_argparse() {
        let help = r#"usage: example [-h] [-v]

A tool

options:
  -h, --help  show help
  -v, --verbose  verbose
"#;
        let format = ArgparseFormat;
        assert!(format.detect(help) > 0.5);
    }

    #[test]
    fn test_parse_usage_and_name() {
        let help = "usage: example [-h] [-v]\n\nA description\n\noptions:\n  -h, --help  help\n";
        let spec = ArgparseFormat.parse(help).unwrap();
        assert_eq!(spec.name, Some("example".to_string()));
        assert_eq!(spec.usage, Some("example [-h] [-v]".to_string()));
    }

    #[test]
    fn test_parse_options() {
        let help = r#"usage: example [-h] [-v] [-c FILE]

options:
  -h, --help         show help
  -v, --verbose      Enable verbose
  -c, --config FILE  Config file
"#;
        let spec = ArgparseFormat.parse(help).unwrap();
        assert_eq!(spec.options.len(), 2); // help filtered out
        assert_eq!(spec.options[0].short, Some("-v".to_string()));
        assert_eq!(spec.options[1].value, Some("<FILE>".to_string()));
    }

    #[test]
    fn test_parse_default_value() {
        let help = r#"usage: example

options:
  -p, --port PORT  Port number (default: 8080)
"#;
        let spec = ArgparseFormat.parse(help).unwrap();
        assert_eq!(spec.options[0].default, Some("8080".to_string()));
    }
}
