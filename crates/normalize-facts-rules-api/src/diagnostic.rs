//! Diagnostic output from rules.
//!
//! Rules produce diagnostics when they detect issues in the code.
//! These are displayed to users and can be used for CI enforcement.

use serde::{Deserialize, Serialize};

/// Severity level for a diagnostic.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DiagnosticLevel {
    /// Informational hint
    Hint,
    /// Warning (may indicate a problem)
    Warning,
    /// Error (definite problem)
    Error,
}

/// A source code location.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Location {
    /// File path relative to project root
    pub file: String,
    /// Line number (1-indexed)
    pub line: u32,
    /// Column number (1-indexed, optional)
    pub column: Option<u32>,
}

impl Location {
    /// Create a location with file and line
    pub fn new(file: &str, line: u32) -> Self {
        Self {
            file: file.into(),
            line,
            column: None,
        }
    }

    /// Create a location with file, line, and column
    pub fn with_column(file: &str, line: u32, column: u32) -> Self {
        Self {
            file: file.into(),
            line,
            column: Some(column),
        }
    }
}

/// A diagnostic produced by a rule.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Diagnostic {
    /// Rule ID that produced this diagnostic (e.g., "circular-dependency")
    pub rule_id: String,
    /// Severity level
    pub level: DiagnosticLevel,
    /// Human-readable message
    pub message: String,
    /// Primary location (where the issue was detected)
    pub location: Option<Location>,
    /// Related locations (e.g., other files in a cycle)
    pub related: Vec<Location>,
    /// Optional fix suggestion
    pub suggestion: Option<String>,
}

impl Diagnostic {
    /// Create a new diagnostic
    pub fn new(rule_id: &str, level: DiagnosticLevel, message: &str) -> Self {
        Self {
            rule_id: rule_id.into(),
            level,
            message: message.into(),
            location: None,
            related: Vec::new(),
            suggestion: None,
        }
    }

    /// Create an error diagnostic
    pub fn error(rule_id: &str, message: &str) -> Self {
        Self::new(rule_id, DiagnosticLevel::Error, message)
    }

    /// Create a warning diagnostic
    pub fn warning(rule_id: &str, message: &str) -> Self {
        Self::new(rule_id, DiagnosticLevel::Warning, message)
    }

    /// Create a hint diagnostic
    pub fn hint(rule_id: &str, message: &str) -> Self {
        Self::new(rule_id, DiagnosticLevel::Hint, message)
    }

    /// Set the primary location
    pub fn at(mut self, file: &str, line: u32) -> Self {
        self.location = Some(Location::new(file, line));
        self
    }

    /// Add a related location
    pub fn with_related(mut self, file: &str, line: u32) -> Self {
        self.related.push(Location::new(file, line));
        self
    }

    /// Add a fix suggestion
    pub fn with_suggestion(mut self, suggestion: &str) -> Self {
        self.suggestion = Some(suggestion.into());
        self
    }
}
