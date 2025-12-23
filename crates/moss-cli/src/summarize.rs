//! Module summarization.
//!
//! Generates a brief summary of what a module does based on its
//! public API (exports, main class/function names, docstrings).

use std::path::Path;

use crate::deps::DepsExtractor;
use crate::skeleton::SkeletonExtractor;

/// Summary of a module
#[derive(Debug)]
#[allow(dead_code)] // Fields used by Debug trait
pub struct ModuleSummary {
    pub file_path: String,
    pub module_name: String,
    pub purpose: String,
    pub main_exports: Vec<ExportSummary>,
    pub dependencies: Vec<String>,
    pub line_count: usize,
}

/// Summary of an exported symbol
#[derive(Debug)]
pub struct ExportSummary {
    pub name: String,
    pub kind: String,
    pub signature: Option<String>,
    pub docstring: Option<String>,
}

impl ModuleSummary {
    pub fn format(&self) -> String {
        let mut lines = Vec::new();

        lines.push(format!("# {}", self.module_name));
        lines.push(String::new());
        lines.push(self.purpose.clone());
        lines.push(String::new());

        if !self.main_exports.is_empty() {
            lines.push("## Exports".to_string());
            for exp in &self.main_exports {
                let doc = exp.docstring.as_ref().and_then(|d| {
                    let first = d.lines().next().unwrap_or("");
                    if first.is_empty() {
                        None
                    } else {
                        Some(first.to_string())
                    }
                });

                // Format based on kind
                let line = match exp.kind.as_str() {
                    "class" => format!("  class {}", exp.name),
                    "function" | "method" => {
                        // Extract just args from signature if present
                        if let Some(sig) = &exp.signature {
                            if sig.contains('(') {
                                let args = sig
                                    .split_once('(')
                                    .map(|(_, rest)| format!("({}", rest))
                                    .unwrap_or_default();
                                format!("  {} {}{}", exp.kind, exp.name, args)
                            } else {
                                format!("  {} {}", exp.kind, exp.name)
                            }
                        } else {
                            format!("  {} {}", exp.kind, exp.name)
                        }
                    }
                    _ => format!("  {} {}", exp.kind, exp.name),
                };

                if let Some(d) = &doc {
                    lines.push(format!("{} - {}", line, d));
                } else {
                    lines.push(line);
                }
            }
            lines.push(String::new());
        }

        if !self.dependencies.is_empty() {
            lines.push("## Dependencies".to_string());
            for dep in &self.dependencies {
                lines.push(format!("  {}", dep));
            }
            lines.push(String::new());
        }

        lines.push(format!("({} lines)", self.line_count));

        lines.join("\n")
    }
}

pub fn summarize_module(path: &Path, content: &str) -> ModuleSummary {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");

    // Get module name from path
    let module_name = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown")
        .to_string();

    let line_count = content.lines().count();

    // Extract skeleton for exports
    let mut skeleton_extractor = SkeletonExtractor::new();
    let skeleton = skeleton_extractor.extract(path, content);

    // Extract dependencies
    let mut deps_extractor = DepsExtractor::new();
    let deps_result = deps_extractor.extract(path, content);

    // Build exports list
    let mut main_exports: Vec<ExportSummary> = Vec::new();
    for sym in &skeleton.symbols {
        // Skip private symbols
        if sym.name.starts_with('_') && !sym.name.starts_with("__") {
            continue;
        }

        main_exports.push(ExportSummary {
            name: sym.name.clone(),
            kind: sym.kind.to_string(),
            signature: if sym.signature.is_empty() {
                None
            } else {
                Some(sym.signature.clone())
            },
            docstring: sym.docstring.clone(),
        });

        // Add children (methods) for classes
        for child in &sym.children {
            if !child.name.starts_with('_') || child.name.starts_with("__") {
                main_exports.push(ExportSummary {
                    name: format!("{}.{}", sym.name, child.name),
                    kind: child.kind.to_string(),
                    signature: if child.signature.is_empty() {
                        None
                    } else {
                        Some(child.signature.clone())
                    },
                    docstring: child.docstring.clone(),
                });
            }
        }
    }

    // Limit to most important exports
    main_exports.truncate(10);

    // Build dependencies list
    let dependencies: Vec<String> = deps_result
        .imports
        .iter()
        .filter(|imp| !imp.is_relative)
        .map(|imp| {
            if imp.names.is_empty() {
                imp.module.clone()
            } else {
                format!("{} ({})", imp.module, imp.names.join(", "))
            }
        })
        .take(10)
        .collect();

    // Generate purpose from module docstring or first class docstring
    let purpose = infer_purpose(&module_name, &main_exports, ext);

    ModuleSummary {
        file_path: path.to_string_lossy().to_string(),
        module_name,
        purpose,
        main_exports,
        dependencies,
        line_count,
    }
}

fn infer_purpose(module_name: &str, exports: &[ExportSummary], ext: &str) -> String {
    // Try to get purpose from first docstring
    for exp in exports {
        if let Some(doc) = &exp.docstring {
            let first_line = doc.lines().next().unwrap_or("");
            if !first_line.is_empty() && first_line.len() > 10 {
                return first_line.to_string();
            }
        }
    }

    // Infer from exports
    let export_names: Vec<&str> = exports.iter().map(|e| e.name.as_str()).collect();

    if export_names.is_empty() {
        return format!("{} module", module_name);
    }

    // Look for common patterns
    let has_class = exports.iter().any(|e| e.kind == "class");
    let has_functions = exports.iter().any(|e| e.kind == "function");

    let lang = match ext {
        "py" => "Python",
        "rs" => "Rust",
        _ => "Source",
    };

    if has_class {
        let classes: Vec<&str> = exports
            .iter()
            .filter(|e| e.kind == "class")
            .map(|e| e.name.as_str())
            .collect();
        if classes.len() == 1 {
            return format!("{} module defining {} class", lang, classes[0]);
        }
        return format!(
            "{} module with {} classes: {}",
            lang,
            classes.len(),
            classes.join(", ")
        );
    }

    if has_functions {
        let funcs: Vec<&str> = exports
            .iter()
            .filter(|e| e.kind == "function")
            .map(|e| e.name.as_str())
            .take(3)
            .collect();
        return format!("{} module with functions: {}", lang, funcs.join(", "));
    }

    format!("{} module", module_name)
}
