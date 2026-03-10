//! Test gap analysis - find public functions with no direct test caller.
//!
//! Uses the call graph index to identify public functions that are never
//! directly called from test context. Computes a risk score based on
//! complexity, caller count, and lines of code.

use crate::output::OutputFormatter;
use serde::Serialize;

/// A public function analyzed for test coverage gaps.
#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
pub struct FunctionTestGap {
    /// Function name
    pub name: String,
    /// Parent type/struct name (for methods)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    /// Source file (relative path)
    pub file_path: String,
    /// Start line in file
    pub start_line: usize,
    /// End line in file
    pub end_line: usize,
    /// Cyclomatic complexity
    pub complexity: usize,
    /// Number of non-test callers
    pub caller_count: usize,
    /// Number of direct test callers
    pub test_caller_count: usize,
    /// Lines of code
    pub loc: usize,
    /// Risk score: complexity * ln(callers + 1) * ln(loc + 1)
    pub risk: f64,
    /// Whether risk was reduced by de-prioritization (x0.1)
    pub de_prioritized: bool,
    /// Reason for de-prioritization
    #[serde(skip_serializing_if = "Option::is_none")]
    pub de_priority_reason: Option<String>,
}

impl FunctionTestGap {
    /// Qualified name for allowlist matching: file_path:Parent.name or file_path:name
    pub fn qualified_name(&self) -> String {
        let base = self.short_name();
        format!("{}:{}", self.file_path, base)
    }

    /// Display name: Parent.name or name
    pub fn short_name(&self) -> String {
        if let Some(ref parent) = self.parent {
            format!("{}.{}", parent, self.name)
        } else {
            self.name.clone()
        }
    }
}

/// Compute risk score for an untested function.
///
/// Formula: complexity * ln(callers + 1) * ln(loc + 1)
/// Logarithmic scaling prevents one extreme value from dominating.
pub fn compute_risk(complexity: usize, caller_count: usize, loc: usize) -> f64 {
    let c = complexity as f64;
    let callers_factor = ((caller_count as f64) + 1.0).ln();
    let loc_factor = ((loc as f64) + 1.0).ln();
    c * callers_factor * loc_factor
}

/// De-prioritization categories for lower-risk untested functions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub enum DePriorityReason {
    /// new, default, from, from_str, try_from
    Constructor,
    /// complexity <= 1, LOC <= 3
    GetterSetter,
    /// Display::fmt, Debug::fmt
    DisplayDebugImpl,
}

impl DePriorityReason {
    pub fn as_str(&self) -> &'static str {
        match self {
            DePriorityReason::Constructor => "constructor",
            DePriorityReason::GetterSetter => "getter/setter",
            DePriorityReason::DisplayDebugImpl => "Display/Debug impl",
        }
    }
}

/// Check if a function should be de-prioritized (risk * 0.1).
pub fn check_de_priority(
    name: &str,
    parent: Option<&str>,
    complexity: usize,
    loc: usize,
) -> Option<DePriorityReason> {
    // Constructors
    if matches!(name, "new" | "default" | "from" | "from_str" | "try_from") {
        return Some(DePriorityReason::Constructor);
    }

    // Getters/setters: trivial body
    if complexity <= 1 && loc <= 3 {
        return Some(DePriorityReason::GetterSetter);
    }

    // Display/Debug implementations
    if name == "fmt"
        && let Some(p) = parent
        && (p.contains("Display") || p.contains("Debug"))
    {
        return Some(DePriorityReason::DisplayDebugImpl);
    }

    None
}

/// Full report for test gaps analysis.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct TestGapsReport {
    /// Functions analyzed (sorted by risk desc for untested, then test count asc)
    pub functions: Vec<FunctionTestGap>,
    /// Total public functions analyzed (before allowlist)
    pub total_public: usize,
    /// Number with zero test callers
    pub untested_count: usize,
    /// Number excluded via allowlist
    pub allowed_count: usize,
    /// Whether --all mode was used
    pub show_all: bool,
}

impl OutputFormatter for TestGapsReport {
    fn format_text(&self) -> String {
        let mut lines = Vec::new();

        lines.push(format!(
            "Test Gaps: {} of {} public functions have no direct test",
            self.untested_count, self.total_public
        ));

        if self.allowed_count > 0 {
            lines.push(format!("Allowed: {} functions", self.allowed_count));
        }

        if self.functions.is_empty() {
            return lines.join("\n");
        }

        lines.push(String::new());

        if self.show_all {
            lines.push(format!(
                " {:>5}  {:>6}  {:<36}  {:<24}  {:>10}  {:>7}  {:>3}",
                "Tests", "Risk", "Function", "File", "Complexity", "Callers", "LOC"
            ));
            lines.push(format!(
                " {:─>5}  {:─>6}  {:─<36}  {:─<24}  {:─>10}  {:─>7}  {:─>3}",
                "", "", "", "", "", "", ""
            ));

            for func in &self.functions {
                let risk_str = if func.test_caller_count == 0 {
                    format!("{:.1}", func.risk)
                } else {
                    "-".to_string()
                };
                let location = format!("{}:{}", func.file_path, func.start_line);
                lines.push(format!(
                    " {:>5}  {:>6}  {:<36}  {:<24}  {:>10}  {:>7}  {:>3}",
                    func.test_caller_count,
                    risk_str,
                    truncate_str(&func.short_name(), 36),
                    truncate_str(&location, 24),
                    func.complexity,
                    func.caller_count,
                    func.loc,
                ));
            }
        } else {
            lines.push(format!(
                " {:>6}  {:<36}  {:<24}  {:>10}  {:>7}  {:>3}",
                "Risk", "Function", "File", "Complexity", "Callers", "LOC"
            ));
            lines.push(format!(
                " {:─>6}  {:─<36}  {:─<24}  {:─>10}  {:─>7}  {:─>3}",
                "", "", "", "", "", ""
            ));

            for func in &self.functions {
                if func.test_caller_count > 0 {
                    continue;
                }
                let location = format!("{}:{}", func.file_path, func.start_line);
                lines.push(format!(
                    " {:>6.1}  {:<36}  {:<24}  {:>10}  {:>7}  {:>3}",
                    func.risk,
                    truncate_str(&func.short_name(), 36),
                    truncate_str(&location, 24),
                    func.complexity,
                    func.caller_count,
                    func.loc,
                ));
            }
        }

        lines.join("\n")
    }
}

/// Truncate a string to max_len, adding "..." if truncated.
fn truncate_str(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else if max_len > 3 {
        format!("{}...", &s[..max_len - 3])
    } else {
        s[..max_len].to_string()
    }
}
