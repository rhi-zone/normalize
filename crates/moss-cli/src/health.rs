//! Codebase health metrics.
//!
//! Quick overview of codebase health including file counts,
//! complexity summary, and structural metrics.

use std::path::Path;

use crate::complexity::ComplexityAnalyzer;
use crate::path_resolve;

/// Health metrics for a codebase
#[derive(Debug)]
pub struct HealthReport {
    pub total_files: usize,
    pub python_files: usize,
    pub rust_files: usize,
    pub other_files: usize,
    pub total_lines: usize,
    pub avg_complexity: f64,
    pub max_complexity: usize,
    pub high_risk_functions: usize,
    pub total_functions: usize,
}

impl HealthReport {
    pub fn format(&self) -> String {
        let mut lines = Vec::new();

        lines.push("# Codebase Health".to_string());
        lines.push(String::new());

        lines.push("## Files".to_string());
        lines.push(format!("  Total: {}", self.total_files));
        lines.push(format!("  Python: {}", self.python_files));
        lines.push(format!("  Rust: {}", self.rust_files));
        lines.push(format!("  Other: {}", self.other_files));
        lines.push(format!("  Lines: {}", self.total_lines));
        lines.push(String::new());

        lines.push("## Complexity".to_string());
        lines.push(format!("  Functions: {}", self.total_functions));
        lines.push(format!("  Average: {:.1}", self.avg_complexity));
        lines.push(format!("  Maximum: {}", self.max_complexity));
        lines.push(format!("  High risk (>10): {}", self.high_risk_functions));

        let health_score = self.calculate_health_score();
        let grade = self.grade();
        lines.push(String::new());
        lines.push(format!("## Score: {} ({:.0}%)", grade, health_score * 100.0));

        lines.join("\n")
    }

    fn calculate_health_score(&self) -> f64 {
        // Simple scoring based on complexity
        // Lower average complexity = better
        // Lower high-risk ratio = better

        let complexity_score = if self.avg_complexity <= 3.0 {
            1.0
        } else if self.avg_complexity <= 5.0 {
            0.9
        } else if self.avg_complexity <= 7.0 {
            0.8
        } else if self.avg_complexity <= 10.0 {
            0.7
        } else if self.avg_complexity <= 15.0 {
            0.5
        } else {
            0.3
        };

        let high_risk_ratio = if self.total_functions > 0 {
            self.high_risk_functions as f64 / self.total_functions as f64
        } else {
            0.0
        };

        let risk_score = if high_risk_ratio <= 0.01 {
            1.0
        } else if high_risk_ratio <= 0.02 {
            0.9
        } else if high_risk_ratio <= 0.03 {
            0.8
        } else if high_risk_ratio <= 0.05 {
            0.7
        } else if high_risk_ratio <= 0.1 {
            0.5
        } else {
            0.3
        };

        (complexity_score + risk_score) / 2.0
    }

    fn grade(&self) -> &'static str {
        let score = self.calculate_health_score();
        if score >= 0.9 {
            "A"
        } else if score >= 0.8 {
            "B"
        } else if score >= 0.7 {
            "C"
        } else if score >= 0.6 {
            "D"
        } else {
            "F"
        }
    }
}

pub fn analyze_health(root: &Path) -> HealthReport {
    let all_files = path_resolve::all_files(root);

    let mut python_files = 0;
    let mut rust_files = 0;
    let mut other_files = 0;
    let mut total_lines = 0;

    let mut total_functions = 0;
    let mut total_complexity: usize = 0;
    let mut max_complexity: usize = 0;
    let mut high_risk_functions = 0;

    let mut analyzer = ComplexityAnalyzer::new();

    for file in all_files.iter().filter(|f| f.kind == "file") {
        let path = root.join(&file.path);
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");

        match ext {
            "py" => python_files += 1,
            "rs" => rust_files += 1,
            _ => other_files += 1,
        }

        // Count lines
        if let Ok(content) = std::fs::read_to_string(&path) {
            total_lines += content.lines().count();

            // Analyze complexity for Python/Rust
            if ext == "py" || ext == "rs" {
                let report = analyzer.analyze(&path, &content);
                for func in &report.functions {
                    total_functions += 1;
                    total_complexity += func.complexity;
                    if func.complexity > max_complexity {
                        max_complexity = func.complexity;
                    }
                    if func.complexity > 10 {
                        high_risk_functions += 1;
                    }
                }
            }
        }
    }

    let avg_complexity = if total_functions > 0 {
        total_complexity as f64 / total_functions as f64
    } else {
        0.0
    };

    HealthReport {
        total_files: python_files + rust_files + other_files,
        python_files,
        rust_files,
        other_files,
        total_lines,
        avg_complexity,
        max_complexity,
        high_risk_functions,
        total_functions,
    }
}
