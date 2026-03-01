//! Output formatting utilities.
//!
//! Provides consistent JSON/text output across all commands via the `OutputFormatter` trait.

use normalize_core::Merge;
use normalize_derive::Merge;
use serde::{Deserialize, Serialize};
use std::io::IsTerminal;

/// Color output mode.
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize, schemars::JsonSchema,
)]
#[serde(rename_all = "lowercase")]
pub enum ColorMode {
    /// Auto-detect based on TTY (default)
    #[default]
    Auto,
    /// Always use colors
    Always,
    /// Never use colors
    Never,
}

impl Merge for ColorMode {
    fn merge(self, other: Self) -> Self {
        other
    }
}

/// Configuration for pretty output mode.
///
/// Example config.toml:
/// ```toml
/// [pretty]
/// enabled = true       # auto-enable when TTY (default: auto)
/// colors = "auto"      # "auto", "always", or "never"
/// highlight = true     # syntax highlighting on signatures
/// ```
#[derive(Debug, Clone, Deserialize, Serialize, Merge, Default, schemars::JsonSchema)]
#[serde(default)]
pub struct PrettyConfig {
    /// Enable pretty mode. None = auto (true when stdout is TTY)
    pub enabled: Option<bool>,
    /// Color mode: auto (default), always, or never
    pub colors: Option<ColorMode>,
    /// Enable syntax highlighting. Default: true
    pub highlight: Option<bool>,
}

impl PrettyConfig {
    /// Should pretty mode be enabled?
    /// Respects explicit setting, otherwise auto-detects TTY.
    pub fn enabled(&self) -> bool {
        self.enabled
            .unwrap_or_else(|| std::io::stdout().is_terminal())
    }

    /// Should colors be used?
    /// Respects colors setting and NO_COLOR env var.
    pub fn use_colors(&self) -> bool {
        // Check NO_COLOR env var first (standard)
        if std::env::var("NO_COLOR").is_ok() {
            return false;
        }

        match self.colors.unwrap_or_default() {
            ColorMode::Always => true,
            ColorMode::Never => false,
            ColorMode::Auto => std::io::stdout().is_terminal(),
        }
    }

    /// Should syntax highlighting be used?
    pub fn highlight(&self) -> bool {
        self.highlight.unwrap_or(true)
    }
}

/// Output format and display mode.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum OutputFormat {
    /// Compact text output (LLM-optimized, no colors).
    #[default]
    Compact,
    /// Pretty text output (human-friendly, with colors if available).
    Pretty { colors: bool },
    /// JSON output.
    Json,
    /// JSON Lines output (one JSON object per line, arrays emit each element).
    JsonLines,
    /// JSON filtered through jq expression. If jsonl is true, emit results as JSON Lines.
    Jq { filter: String, jsonl: bool },
}

impl OutputFormat {
    /// Create from CLI flags and config (fully resolved).
    pub fn from_cli(
        json: bool,
        jsonl: bool,
        jq: Option<&str>,
        pretty: bool,
        compact: bool,
        config: &PrettyConfig,
    ) -> Self {
        // JSON modes take precedence
        if let Some(filter) = jq {
            return OutputFormat::Jq {
                filter: filter.to_string(),
                jsonl,
            };
        }
        if jsonl {
            return OutputFormat::JsonLines;
        }
        if json {
            return OutputFormat::Json;
        }

        // Determine text mode
        let is_pretty = if compact {
            false
        } else {
            pretty || config.enabled()
        };

        if is_pretty {
            // Determine colors: respect "never", otherwise --pretty forces colors
            let use_colors = if std::env::var("NO_COLOR").is_ok() {
                false
            } else {
                match config.colors.unwrap_or_default() {
                    ColorMode::Never => false,
                    ColorMode::Always => true,
                    ColorMode::Auto => {
                        // Explicit --pretty overrides TTY check
                        pretty || std::io::stdout().is_terminal()
                    }
                }
            };
            OutputFormat::Pretty { colors: use_colors }
        } else {
            OutputFormat::Compact
        }
    }

    /// Is this a JSON-based format?
    pub fn is_json(&self) -> bool {
        matches!(
            self,
            OutputFormat::Json | OutputFormat::JsonLines | OutputFormat::Jq { .. }
        )
    }

    /// Is this pretty mode?
    pub fn is_pretty(&self) -> bool {
        matches!(self, OutputFormat::Pretty { .. })
    }

    /// Are colors enabled?
    pub fn use_colors(&self) -> bool {
        matches!(self, OutputFormat::Pretty { colors: true })
    }
}

/// Trait for types that can format output in multiple formats.
///
/// Types implementing this trait can be printed as either JSON or text.
/// JSON serialization uses serde, while text formatting is custom.
/// Schema generation uses schemars for `--output-schema` support.
pub trait OutputFormatter: Serialize + schemars::JsonSchema {
    /// Format as minimal text (LLM-optimized, default).
    fn format_text(&self) -> String;

    /// Format as pretty text (human-friendly with colors).
    /// Default implementation falls back to format_text().
    fn format_pretty(&self) -> String {
        self.format_text()
    }

    /// Print to stdout in the specified format.
    fn print(&self, format: &OutputFormat) {
        match format {
            OutputFormat::Compact => println!("{}", self.format_text()),
            OutputFormat::Pretty { .. } => println!("{}", self.format_pretty()),
            OutputFormat::Json => {
                println!("{}", serde_json::to_string(self).unwrap_or_default())
            }
            OutputFormat::JsonLines => {
                let json = serde_json::to_value(self).unwrap_or_default();
                print_jsonl(&json);
            }
            OutputFormat::Jq { filter, jsonl } => {
                let json = serde_json::to_value(self).unwrap_or_default();
                match apply_jq(&json, filter) {
                    Ok(results) => {
                        for result in results {
                            if *jsonl {
                                // Parse and emit as JSONL (arrays get expanded)
                                if let Ok(val) = serde_json::from_str::<serde_json::Value>(&result)
                                {
                                    print_jsonl(&val);
                                } else {
                                    println!("{}", result);
                                }
                            } else {
                                println!("{}", result);
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("jq error: {}", e);
                    }
                }
            }
        }
    }
}

/// Print JSON value as JSON Lines (one object per line).
/// Arrays emit each element as a separate line, other values emit as single line.
fn print_jsonl(value: &serde_json::Value) {
    if let serde_json::Value::Array(arr) = value {
        for item in arr {
            println!("{}", serde_json::to_string(item).unwrap_or_default());
        }
    } else {
        println!("{}", serde_json::to_string(value).unwrap_or_default());
    }
}

/// Print JSON schema for a type implementing OutputFormatter.
/// Use this for `--output-schema` flag handling.
pub fn print_output_schema<T: OutputFormatter>() {
    let schema = schemars::schema_for!(T);
    println!(
        "{}",
        serde_json::to_string_pretty(&schema).unwrap_or_default()
    );
}

/// Apply a jq filter to a JSON value.
pub fn apply_jq(value: &serde_json::Value, filter: &str) -> Result<Vec<String>, String> {
    use jaq_core::load::{Arena, File as JaqFile, Loader};
    use jaq_core::{Compiler, Ctx, RcIter};
    use jaq_json::Val;

    let loader = Loader::new(jaq_std::defs().chain(jaq_json::defs()));
    let arena = Arena::default();

    let program = JaqFile {
        code: filter,
        path: (),
    };

    let modules = loader
        .load(&arena, program)
        .map_err(|errs| format!("jq parse error: {:?}", errs))?;

    let filter_compiled = Compiler::default()
        .with_funs(jaq_std::funs().chain(jaq_json::funs()))
        .compile(modules)
        .map_err(|errs| format!("jq compile error: {:?}", errs))?;

    let val = Val::from(value.clone());
    let inputs = RcIter::new(core::iter::empty());
    let out = filter_compiled.run((Ctx::new([], &inputs), val));

    let mut results = Vec::new();
    for result in out {
        match result {
            Ok(v) => results.push(v.to_string()),
            Err(e) => return Err(format!("jq runtime error: {:?}", e)),
        }
    }

    Ok(results)
}

/// Print jq-filtered lines to stdout.
///
/// When `jsonl` is true, each line is parsed as JSON: arrays are exploded
/// one item per line, other values are emitted as compact JSON. When false,
/// each line is printed as-is.
pub fn print_jq_lines(lines: &[String], jsonl: bool) {
    for line in lines {
        if jsonl {
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(line) {
                if let serde_json::Value::Array(arr) = val {
                    for item in arr {
                        println!("{}", serde_json::to_string(&item).unwrap_or_default());
                    }
                } else {
                    println!("{}", serde_json::to_string(&val).unwrap_or_default());
                }
            } else {
                println!("{}", line);
            }
        } else {
            println!("{}", line);
        }
    }
}

/// Render a plain (uncolored) progress bar using block characters.
///
/// `ratio` is clamped to 0.0–1.0. `width` is the total character count.
/// Callers can wrap the result in ANSI color as needed.
pub fn progress_bar(ratio: f64, width: usize) -> String {
    let ratio = ratio.clamp(0.0, 1.0);
    let filled = (ratio * width as f64).round() as usize;
    format!("{}{}", "█".repeat(filled), "░".repeat(width - filled))
}

/// Render a colored progress bar where high ratio = good (green) and low = bad (red).
pub fn progress_bar_good(ratio: f64, width: usize) -> String {
    use nu_ansi_term::Color;
    let color = if ratio >= 0.67 {
        Color::Green
    } else if ratio >= 0.34 {
        Color::Yellow
    } else {
        Color::Red
    };
    color.paint(progress_bar(ratio, width)).to_string()
}

/// Render a colored progress bar where high ratio = bad (red) and low = good (green).
pub fn progress_bar_bad(ratio: f64, width: usize) -> String {
    progress_bar_good(1.0 - ratio, width)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Serialize, schemars::JsonSchema)]
    #[allow(dead_code)]
    struct TestOutput {
        name: String,
        count: usize,
    }

    impl OutputFormatter for TestOutput {
        fn format_text(&self) -> String {
            format!("{}: {}", self.name, self.count)
        }
    }

    #[test]
    fn test_output_format_from_cli() {
        let config = PrettyConfig::default();
        // compact=true overrides auto
        assert_eq!(
            OutputFormat::from_cli(false, false, None, false, true, &config),
            OutputFormat::Compact
        );
        assert_eq!(
            OutputFormat::from_cli(true, false, None, false, false, &config),
            OutputFormat::Json
        );
        assert_eq!(
            OutputFormat::from_cli(false, true, None, false, false, &config),
            OutputFormat::JsonLines
        );
        assert_eq!(
            OutputFormat::from_cli(false, false, Some(".name"), false, false, &config),
            OutputFormat::Jq {
                filter: ".name".to_string(),
                jsonl: false
            }
        );
        // jq + jsonl
        assert_eq!(
            OutputFormat::from_cli(true, true, Some(".name"), false, false, &config),
            OutputFormat::Jq {
                filter: ".name".to_string(),
                jsonl: true
            }
        );
        // jsonl takes precedence over json (when no jq)
        assert_eq!(
            OutputFormat::from_cli(true, true, None, false, false, &config),
            OutputFormat::JsonLines
        );
        // jq without jsonl
        assert_eq!(
            OutputFormat::from_cli(true, false, Some(".name"), false, false, &config),
            OutputFormat::Jq {
                filter: ".name".to_string(),
                jsonl: false
            }
        );
    }

    #[test]
    fn test_apply_jq() {
        let value = serde_json::json!({"name": "test", "count": 42});
        let results = apply_jq(&value, ".name").unwrap();
        assert_eq!(results, vec!["\"test\""]);

        let results = apply_jq(&value, ".count").unwrap();
        assert_eq!(results, vec!["42"]);
    }

    #[test]
    fn test_color_mode_merge() {
        // Later value wins
        assert_eq!(ColorMode::Auto.merge(ColorMode::Always), ColorMode::Always);
        assert_eq!(ColorMode::Always.merge(ColorMode::Never), ColorMode::Never);
        assert_eq!(ColorMode::Never.merge(ColorMode::Auto), ColorMode::Auto);
    }

    #[test]
    fn test_pretty_config_use_colors() {
        // Always mode
        let config = PrettyConfig {
            colors: Some(ColorMode::Always),
            ..Default::default()
        };
        assert!(config.use_colors());

        // Never mode
        let config = PrettyConfig {
            colors: Some(ColorMode::Never),
            ..Default::default()
        };
        assert!(!config.use_colors());
    }

    #[test]
    fn test_pretty_config_highlight() {
        // Default is true
        let config = PrettyConfig::default();
        assert!(config.highlight());

        // Explicit false
        let config = PrettyConfig {
            highlight: Some(false),
            ..Default::default()
        };
        assert!(!config.highlight());
    }
}
