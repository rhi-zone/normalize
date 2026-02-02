//! Parser for clap/structopt-style --help output.
//!
//! Clap format characteristics:
//! - First line: `<name> <version>` OR description text
//! - If first line is description, name comes from Usage line
//! - `Usage: <name> [OPTIONS] ...`
//! - `Commands:` section with `  <name>  <description>`
//! - `Options:` section with `  -s, --long <VALUE>  Description`

use super::CliFormat;
use crate::{CliCommand, CliOption, CliSpec};
use regex::Regex;

/// Parser for clap-style CLI help output.
pub struct ClapFormat;

impl CliFormat for ClapFormat {
    fn name(&self) -> &'static str {
        "clap"
    }

    fn detect(&self, help_text: &str) -> f64 {
        let mut score: f64 = 0.0;

        // Check for "Usage:" line (very common in clap)
        if help_text.contains("Usage:") {
            score += 0.3;
        }

        // Check for "Commands:" section
        if help_text.contains("\nCommands:\n") || help_text.contains("\nCommands:\r\n") {
            score += 0.2;
        }

        // Check for "Options:" section
        if help_text.contains("\nOptions:\n") || help_text.contains("\nOptions:\r\n") {
            score += 0.2;
        }

        // Check for typical clap option format: "  -x, --xxx"
        if Regex::new(r"^\s+-\w,\s+--\w").unwrap().is_match(help_text) {
            score += 0.2;
        }

        // Check for "[OPTIONS]" in usage
        if help_text.contains("[OPTIONS]") {
            score += 0.1;
        }

        score.min(1.0)
    }

    fn parse(&self, help_text: &str) -> Result<CliSpec, String> {
        let mut spec = CliSpec::default();
        let lines: Vec<&str> = help_text.lines().collect();

        if lines.is_empty() {
            return Err("Empty help text".to_string());
        }

        // First pass: find Usage line to extract name
        for line in &lines {
            if line.starts_with("Usage:") {
                let usage = line.trim_start_matches("Usage:").trim();
                spec.usage = Some(usage.to_string());
                // Extract name from "Usage: <name> [OPTIONS]..."
                if let Some(name) = usage.split_whitespace().next() {
                    spec.name = Some(name.to_string());
                }
                break;
            }
        }

        let mut i = 0;

        // Check if first line is "name version" format
        if let Some(first_line) = lines.first() {
            let parts: Vec<&str> = first_line.split_whitespace().collect();
            if parts.len() == 2 {
                let potential_version = parts[1];
                // If second part looks like a version, first line is name+version
                if potential_version
                    .chars()
                    .next()
                    .is_some_and(|c| c.is_ascii_digit())
                    || potential_version.starts_with('v')
                {
                    spec.name = Some(parts[0].to_string());
                    spec.version = Some(potential_version.to_string());
                    i += 1;
                }
            }
        }

        // Parse description (lines until Usage: or Commands: or Options:)
        let mut description_lines = Vec::new();
        while i < lines.len() {
            let line = lines[i];
            if line.starts_with("Usage:")
                || line == "Commands:"
                || line == "Options:"
                || line == "Arguments:"
            {
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

            if line.starts_with("Usage:") {
                // Already parsed above, just skip
                i += 1;
            } else if line == "Commands:" {
                i += 1;
                while i < lines.len() && !is_section_header(lines[i]) {
                    if let Some(cmd) = parse_command_line(lines[i]) {
                        spec.commands.push(cmd);
                    }
                    i += 1;
                }
            } else if line == "Options:" || line == "Arguments:" {
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

/// Check if a line is a section header (e.g., "Commands:", "Options:").
fn is_section_header(line: &str) -> bool {
    let trimmed = line.trim();
    !trimmed.is_empty()
        && !trimmed.starts_with('-')
        && !trimmed.starts_with(' ')
        && trimmed.ends_with(':')
}

/// Parse a command line like "  run   Run something".
fn parse_command_line(line: &str) -> Option<CliCommand> {
    let trimmed = line.trim();
    if trimmed.is_empty() || trimmed.starts_with('-') {
        return None;
    }

    // Split on multiple spaces to separate name from description
    let re = Regex::new(r"^(\S+)\s{2,}(.*)$").unwrap();
    if let Some(caps) = re.captures(trimmed) {
        let name = caps.get(1)?.as_str().to_string();
        let description = caps.get(2).map(|m| m.as_str().to_string());

        // Skip "help" command as it's meta
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
        // Just a command name with no description
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

/// Parse an option line like "  -v, --verbose  Enable verbose output".
fn parse_option_line(line: &str) -> Option<CliOption> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return None;
    }

    // Patterns:
    // "-v, --verbose  Description"
    // "--verbose  Description"
    // "-v  Description"
    // "-v, --verbose <VALUE>  Description"
    // "--config <FILE>  Description"

    let mut opt = CliOption {
        short: None,
        long: None,
        value: None,
        description: None,
        default: None,
        required: false,
        env: None,
    };

    // Regex to match option patterns
    // Group 1: short flag (-x)
    // Group 2: long flag (--xxx)
    // Group 3: value (<VALUE> or [VALUE])
    // Group 4: description (after 2+ spaces)
    let re =
        Regex::new(r"^(-\w)?(?:,\s*)?(--[\w-]+)?(?:\s*(<[^>]+>|\[[^\]]+\]))?\s{2,}(.*)$").unwrap();

    if let Some(caps) = re.captures(trimmed) {
        opt.short = caps.get(1).map(|m| m.as_str().to_string());
        opt.long = caps.get(2).map(|m| m.as_str().to_string());
        opt.value = caps.get(3).map(|m| m.as_str().to_string());
        opt.description = caps.get(4).map(|m| m.as_str().to_string());

        // Check for default value in description
        if let Some(ref desc) = opt.description {
            if let Some(start) = desc.find("[default:")
                && let Some(end) = desc[start..].find(']')
            {
                let default = desc[start + 9..start + end].trim().to_string();
                opt.default = Some(default);
            }
            // Check for env var
            if let Some(start) = desc.find("[env:")
                && let Some(end) = desc[start..].find(']')
            {
                let env = desc[start + 5..start + end].trim().to_string();
                opt.env = Some(env);
            }
        }

        // Skip help/version as they're meta
        if opt.long == Some("--help".to_string()) || opt.long == Some("--version".to_string()) {
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
    fn test_detect_clap() {
        let help = r#"mycli 1.0.0
A tool

Usage: mycli [OPTIONS]

Options:
  -v, --verbose  Verbose
  -h, --help     Print help
"#;
        let format = ClapFormat;
        assert!(format.detect(help) > 0.5);
    }

    #[test]
    fn test_parse_name_version() {
        let help = "mycli 1.0.0\nA description\n\nUsage: mycli\n";
        let spec = ClapFormat.parse(help).unwrap();
        assert_eq!(spec.name, Some("mycli".to_string()));
        assert_eq!(spec.version, Some("1.0.0".to_string()));
    }

    #[test]
    fn test_parse_options() {
        let help = r#"mycli 1.0.0

Usage: mycli [OPTIONS]

Options:
  -v, --verbose        Enable verbose output
  -c, --config <FILE>  Config file path
"#;
        let spec = ClapFormat.parse(help).unwrap();
        assert_eq!(spec.options.len(), 2);
        assert_eq!(spec.options[0].short, Some("-v".to_string()));
        assert_eq!(spec.options[0].long, Some("--verbose".to_string()));
        assert_eq!(spec.options[1].value, Some("<FILE>".to_string()));
    }

    #[test]
    fn test_parse_commands() {
        let help = r#"mycli 1.0.0

Usage: mycli <COMMAND>

Commands:
  run    Run the thing
  build  Build the thing
  help   Print help
"#;
        let spec = ClapFormat.parse(help).unwrap();
        assert_eq!(spec.commands.len(), 2); // help is filtered out
        assert_eq!(spec.commands[0].name, "run");
        assert_eq!(spec.commands[1].name, "build");
    }

    #[test]
    fn test_parse_default_value() {
        let help = r#"mycli 1.0.0

Options:
  -p, --port <PORT>  Port number [default: 8080]
"#;
        let spec = ClapFormat.parse(help).unwrap();
        assert_eq!(spec.options[0].default, Some("8080".to_string()));
    }
}
