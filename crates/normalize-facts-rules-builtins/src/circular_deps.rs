//! Circular dependency detection using Datalog.
//!
//! This rule detects cycles in the import graph:
//! - A imports B, B imports A (direct cycle)
//! - A imports B, B imports C, C imports A (transitive cycle)
//!
//! Implemented using Ascent's Datalog engine for declarative pattern matching.

use ascent::ascent;
use normalize_facts_rules_api::{Diagnostic, DiagnosticLevel, Relations};
use std::collections::HashSet;

// Define the Datalog program for cycle detection
ascent! {
    /// Cycle detection program
    struct CycleDetector;

    // Input relation: direct imports between files
    relation imports(String, String);  // (from_file, to_file)

    // Derived relation: transitive reachability
    relation reaches(String, String);  // (from, to) where from can reach to

    // Derived relation: detected cycles
    relation cycle(String, String);    // (file_a, file_b) where a cycle exists

    // Base case: direct import means reachability
    reaches(from, to) <-- imports(from, to);

    // Recursive case: transitive reachability
    reaches(from, to) <-- imports(from, mid), reaches(mid, to);

    // Cycle detection: A reaches B and B reaches A
    cycle(a, b) <-- reaches(a, b), reaches(b, a), if a < b;
}

/// Run the circular dependency rule
pub fn run(relations: &Relations) -> Vec<Diagnostic> {
    let mut detector = CycleDetector::default();

    // Populate input relation from imports
    // Group imports by file to get file-level dependencies
    let mut file_imports: HashSet<(String, String)> = HashSet::new();

    for import in relations.imports.iter() {
        let from_file = import.from_file.to_string();
        let to_module = import.to_module.to_string();

        // Only consider local file imports (not external packages)
        // Local imports typically don't start with common package prefixes
        if !to_module.starts_with("std")
            && !to_module.starts_with("http")
            && !to_module.starts_with("node:")
            && !to_module.contains("::")
        {
            file_imports.insert((from_file, to_module));
        }
    }

    // Add imports to Ascent program
    for (from, to) in file_imports {
        detector.imports.push((from, to));
    }

    // Run Datalog to fixed point
    detector.run();

    // Convert cycles to diagnostics
    let mut diagnostics = Vec::new();
    let mut reported: HashSet<(String, String)> = HashSet::new();

    for (file_a, file_b) in detector.cycle.iter() {
        // Normalize pair to avoid duplicate reports
        let pair = if file_a < file_b {
            (file_a.clone(), file_b.clone())
        } else {
            (file_b.clone(), file_a.clone())
        };

        if reported.insert(pair.clone()) {
            let message = format!("Circular dependency: {} ↔ {}", pair.0, pair.1);

            diagnostics.push(
                Diagnostic::new("circular-deps", DiagnosticLevel::Warning, &message)
                    .at(&pair.0, 1)
                    .with_related(&pair.1, 1),
            );
        }
    }

    diagnostics
}

#[cfg(test)]
mod tests {
    use super::*;
    use normalize_facts_rules_api::Relations;

    #[test]
    fn test_no_cycles() {
        let mut relations = Relations::new();
        relations.add_import("a.py", "b", "*");
        relations.add_import("b.py", "c", "*");

        let diagnostics = run(&relations);
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn test_direct_cycle() {
        let mut relations = Relations::new();
        relations.add_import("a.py", "b.py", "*");
        relations.add_import("b.py", "a.py", "*");

        let diagnostics = run(&relations);
        assert_eq!(diagnostics.len(), 1);
        assert!(
            diagnostics[0]
                .message
                .to_string()
                .contains("Circular dependency")
        );
    }

    #[test]
    fn test_transitive_cycle() {
        let mut relations = Relations::new();
        relations.add_import("a.py", "b.py", "*");
        relations.add_import("b.py", "c.py", "*");
        relations.add_import("c.py", "a.py", "*");

        let diagnostics = run(&relations);
        // Should detect at least one cycle relationship
        assert!(!diagnostics.is_empty());
    }

    #[test]
    fn test_ignores_external_imports() {
        let mut relations = Relations::new();
        relations.add_import("a.py", "std::collections", "HashMap");
        relations.add_import("a.py", "http", "*");

        let diagnostics = run(&relations);
        assert!(diagnostics.is_empty());
    }
}
