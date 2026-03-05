//! Output formatting utilities.
//!
//! Provides text formatting via the `OutputFormatter` trait.
//! JSON/jq/jsonl/schema output is handled by server-less at the CLI macro level.

pub mod diagnostics;

use normalize_core::Merge;
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

/// Trait for types that can format output in multiple formats.
///
/// Types implementing this trait provide text formatting. JSON/jq/jsonl
/// output is handled automatically by server-less via `Serialize`.
pub trait OutputFormatter: Serialize + schemars::JsonSchema {
    /// Format as minimal text (LLM-optimized, default).
    fn format_text(&self) -> String;

    /// Format as pretty text (human-friendly with colors).
    /// Default implementation falls back to format_text().
    fn format_pretty(&self) -> String {
        self.format_text()
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
