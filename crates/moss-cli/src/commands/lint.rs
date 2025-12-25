//! Lint command - run linters, formatters, and type checkers.

use moss_tools::{registry_with_custom, SarifReport, ToolCategory, ToolRegistry};
use std::path::Path;

/// Run linting tools on the codebase.
pub fn cmd_lint(
    target: Option<&str>,
    root: Option<&Path>,
    fix: bool,
    tools: Option<&str>,
    category: Option<&str>,
    list: bool,
    sarif: bool,
    json: bool,
) -> i32 {
    let root = root.unwrap_or_else(|| Path::new("."));
    // Load built-in tools + custom tools from .moss/tools.toml
    let registry = registry_with_custom(root);

    // List available tools
    if list {
        return cmd_list_tools(&registry, json);
    }

    // Parse category filter
    let category_filter: Option<ToolCategory> = category.and_then(|c| match c {
        "lint" | "linter" => Some(ToolCategory::Linter),
        "fmt" | "format" | "formatter" => Some(ToolCategory::Formatter),
        "type" | "typecheck" | "type-checker" => Some(ToolCategory::TypeChecker),
        _ => None,
    });

    // Get tools to run
    let tools_to_run: Vec<&dyn moss_tools::Tool> = if let Some(tool_names) = tools {
        // Run specific tools by name
        let names: Vec<&str> = tool_names.split(',').map(|s| s.trim()).collect();
        registry
            .tools()
            .iter()
            .filter(|t| names.contains(&t.info().name))
            .map(|t| t.as_ref())
            .collect()
    } else {
        // Auto-detect relevant tools
        let detected = registry.detect(root);
        detected
            .into_iter()
            .filter(|(t, _)| {
                if let Some(cat) = category_filter {
                    t.info().category == cat
                } else {
                    true
                }
            })
            .map(|(t, _)| t)
            .collect()
    };

    if tools_to_run.is_empty() {
        if json {
            println!("{{\"tools\": [], \"diagnostics\": []}}");
        } else {
            eprintln!("No relevant tools found for this project.");
            eprintln!("Use --list to see available tools.");
        }
        return 0;
    }

    // Prepare paths
    let paths: Vec<&Path> = target.map(|t| vec![Path::new(t)]).unwrap_or_default();

    // Run tools
    let mut all_results = Vec::new();
    let mut had_errors = false;

    for tool in &tools_to_run {
        let info = tool.info();

        if !tool.is_available() {
            if !json {
                eprintln!("{}: not installed", info.name);
            }
            continue;
        }

        if !json {
            let action = if fix && tool.can_fix() {
                "fixing"
            } else {
                "checking"
            };
            eprintln!("{}: {}...", info.name, action);
        }

        let result = if fix && tool.can_fix() {
            tool.fix(&paths.iter().copied().collect::<Vec<_>>(), root)
        } else {
            tool.run(&paths.iter().copied().collect::<Vec<_>>(), root)
        };

        match result {
            Ok(result) => {
                if !result.success {
                    had_errors = true;
                    if let Some(err) = &result.error {
                        if !json {
                            eprintln!("{}: {}", info.name, err);
                        }
                    }
                } else if result.error_count() > 0 {
                    had_errors = true;
                }
                all_results.push(result);
            }
            Err(e) => {
                had_errors = true;
                if !json {
                    eprintln!("{}: {}", info.name, e);
                }
            }
        }
    }

    // Output results
    if sarif {
        let diagnostics = ToolRegistry::collect_diagnostics(&all_results);
        let report = SarifReport::from_diagnostics(&diagnostics);
        println!("{}", serde_json::to_string_pretty(&report).unwrap());
    } else if json {
        let diagnostics = ToolRegistry::collect_diagnostics(&all_results);
        let output = serde_json::json!({
            "tools": tools_to_run.iter().map(|t| {
                let info = t.info();
                serde_json::json!({
                    "name": info.name,
                    "category": info.category.as_str(),
                    "available": t.is_available(),
                    "version": t.version(),
                })
            }).collect::<Vec<_>>(),
            "results": all_results.iter().map(|r| {
                serde_json::json!({
                    "tool": r.tool,
                    "success": r.success,
                    "error_count": r.error_count(),
                    "warning_count": r.warning_count(),
                    "error": r.error,
                })
            }).collect::<Vec<_>>(),
            "diagnostics": diagnostics,
        });
        println!("{}", serde_json::to_string_pretty(&output).unwrap());
    } else {
        // Print diagnostics
        for result in &all_results {
            for diag in &result.diagnostics {
                let severity = match diag.severity {
                    moss_tools::DiagnosticSeverity::Error => "error",
                    moss_tools::DiagnosticSeverity::Warning => "warning",
                    moss_tools::DiagnosticSeverity::Info => "info",
                    moss_tools::DiagnosticSeverity::Hint => "hint",
                };

                println!(
                    "{}:{}:{}: {} [{}] {}",
                    diag.location.file.display(),
                    diag.location.line,
                    diag.location.column,
                    severity,
                    diag.rule_id,
                    diag.message
                );

                if let Some(url) = &diag.help_url {
                    println!("  help: {}", url);
                }
            }
        }

        // Summary
        let total_errors: usize = all_results.iter().map(|r| r.error_count()).sum();
        let total_warnings: usize = all_results.iter().map(|r| r.warning_count()).sum();

        if total_errors > 0 || total_warnings > 0 {
            eprintln!();
            eprintln!(
                "Found {} error(s) and {} warning(s)",
                total_errors, total_warnings
            );
        }
    }

    if had_errors {
        1
    } else {
        0
    }
}

fn cmd_list_tools(registry: &ToolRegistry, json: bool) -> i32 {
    let tools: Vec<_> = registry
        .tools()
        .iter()
        .map(|t| {
            let info = t.info();
            (
                info.name,
                info.category.as_str(),
                t.is_available(),
                t.version(),
                info.extensions.join(", "),
                info.website,
            )
        })
        .collect();

    if json {
        let output: Vec<_> = tools
            .iter()
            .map(
                |(name, category, available, version, extensions, website)| {
                    serde_json::json!({
                        "name": name,
                        "category": category,
                        "available": available,
                        "version": version,
                        "extensions": extensions,
                        "website": website,
                    })
                },
            )
            .collect();
        println!("{}", serde_json::to_string_pretty(&output).unwrap());
    } else {
        println!("Available tools:\n");
        for (name, category, available, version, extensions, website) in tools {
            let status = if available {
                version.unwrap_or_else(|| "installed".to_string())
            } else {
                "not installed".to_string()
            };

            println!("  {} ({})", name, category);
            println!("    Status: {}", status);
            println!("    Extensions: {}", extensions);
            println!("    Website: {}", website);
            println!();
        }
    }

    0
}
