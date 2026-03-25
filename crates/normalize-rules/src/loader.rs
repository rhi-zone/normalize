//! Diagnostic formatting helpers for fact rules.
//!
//! The dylib rule pack loader has been removed. Rules run as interpreted `.dl` files
//! via `normalize-facts-rules-interpret`; there is no dynamic library loading.

use normalize_facts_rules_api::Diagnostic;

/// Format a diagnostic for display
pub fn format_diagnostic(diag: &Diagnostic, use_colors: bool) -> String {
    use normalize_facts_rules_api::DiagnosticLevel;

    let level_str = match diag.level {
        DiagnosticLevel::Hint => {
            if use_colors {
                "\x1b[36mhint\x1b[0m"
            } else {
                "hint"
            }
        }
        DiagnosticLevel::Warning => {
            if use_colors {
                "\x1b[33mwarning\x1b[0m"
            } else {
                "warning"
            }
        }
        DiagnosticLevel::Error => {
            if use_colors {
                "\x1b[31merror\x1b[0m"
            } else {
                "error"
            }
        }
    };

    let mut out = String::new();

    // Location
    if let Some(ref loc) = diag.location {
        out.push_str(&format!("{}:{}: ", loc.file, loc.line));
    }

    // Level and rule
    out.push_str(&format!("{} [{}]: ", level_str, diag.rule_id));

    // Message
    out.push_str(&diag.message);

    // Related locations
    for related in diag.related.iter() {
        out.push_str(&format!("\n  --> {}:{}", related.file, related.line));
    }

    // Suggestion
    if let Some(ref suggestion) = diag.suggestion {
        out.push_str(&format!("\n  suggestion: {}", suggestion));
    }

    out
}
