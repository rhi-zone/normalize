//! Facts management commands (file index, symbols, calls, imports).

use crate::index;
use crate::rules;
use normalize_facts_rules_api::Relations;
use std::io::IsTerminal;
use std::path::{Path, PathBuf};

/// What to extract during indexing (files are always indexed).
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, clap::ValueEnum, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "lowercase")]
pub enum FactsContent {
    /// Skip content extraction (files only)
    None,
    /// Function and type definitions
    Symbols,
    /// Function call relationships
    Calls,
    /// Import statements
    Imports,
}

impl std::fmt::Display for FactsContent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FactsContent::None => write!(f, "none"),
            FactsContent::Symbols => write!(f, "symbols"),
            FactsContent::Calls => write!(f, "calls"),
            FactsContent::Imports => write!(f, "imports"),
        }
    }
}

impl std::str::FromStr for FactsContent {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "none" => Ok(Self::None),
            "symbols" => Ok(Self::Symbols),
            "calls" => Ok(Self::Calls),
            "imports" => Ok(Self::Imports),
            _ => Err(format!("unknown facts content: {s}")),
        }
    }
}

// =============================================================================
// Rules (compiled dylib packs)
// =============================================================================

async fn cmd_rules(
    root: Option<&Path>,
    rule: Option<&str>,
    pack_path: Option<&Path>,
    list: bool,
    json: bool,
) -> i32 {
    let root = root
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));

    // Load rule pack(s)
    let packs: Vec<rules::LoadedRulePack> = if let Some(path) = pack_path {
        // Load specific pack
        match rules::load_from_path(path) {
            Ok(pack) => vec![pack],
            Err(e) => {
                eprintln!("Error loading rule pack: {}", e);
                return 1;
            }
        }
    } else {
        // Discover and load all packs
        let results = rules::load_all(&root);
        let mut loaded = Vec::new();
        for result in results {
            match result {
                Ok(pack) => loaded.push(pack),
                Err(e) => eprintln!("Warning: {}", e),
            }
        }
        loaded
    };

    if packs.is_empty() {
        eprintln!("No rule packs found.");
        eprintln!("Search paths:");
        for path in rules::search_paths(&root) {
            eprintln!("  - {}", path.display());
        }
        eprintln!("\nTo use the builtins, copy the compiled library to one of the search paths.");
        return 1;
    }

    // List mode - just show available rules
    if list {
        if json {
            let all_rules: Vec<_> = packs
                .iter()
                .map(|pack| {
                    let info = pack.info();
                    serde_json::json!({
                        "pack_id": info.id.to_string(),
                        "pack_name": info.name.to_string(),
                        "version": info.version.to_string(),
                        "rules": info.rules.iter().map(|r| r.to_string()).collect::<Vec<_>>(),
                    })
                })
                .collect();
            println!("{}", serde_json::to_string_pretty(&all_rules).unwrap());
        } else {
            for pack in &packs {
                let info = pack.info();
                println!("{} v{}", info.name, info.version);
                println!("  ID: {}", info.id);
                println!("  Path: {}", pack.path.display());
                println!("  Rules:");
                for rule_id in info.rules.iter() {
                    println!("    - {}", rule_id);
                }
                println!();
            }
        }
        return 0;
    }

    // Build relations from index
    let relations = match build_relations_from_index(&root).await {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Error building relations: {}", e);
            eprintln!("Run `normalize facts rebuild` first to index the codebase.");
            return 1;
        }
    };

    // Run rules
    let mut all_diagnostics = Vec::new();
    let use_colors = !json && std::io::stdout().is_terminal();

    for pack in &packs {
        let diagnostics = if let Some(rule_id) = rule {
            pack.run_rule(rule_id, &relations)
        } else {
            pack.run(&relations)
        };
        all_diagnostics.extend(diagnostics);
    }

    // Output results
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&all_diagnostics).unwrap()
        );
    } else if all_diagnostics.is_empty() {
        println!("No issues found.");
    } else {
        for diag in &all_diagnostics {
            println!("{}", rules::format_diagnostic(diag, use_colors));
        }
        println!("\n{} issue(s) found.", all_diagnostics.len());
    }

    if all_diagnostics
        .iter()
        .any(|d| d.level == normalize_facts_rules_api::DiagnosticLevel::Error)
    {
        1
    } else {
        0
    }
}

/// Build Relations from the index
pub async fn build_relations_from_index(root: &Path) -> Result<Relations, String> {
    let idx = index::open(root)
        .await
        .map_err(|e| format!("Failed to open index: {}", e))?;

    let mut relations = Relations::new();

    // Get symbols (file, name, kind, start_line, end_line, parent, visibility)
    let symbols = idx
        .all_symbols_with_details()
        .await
        .map_err(|e| format!("Failed to get symbols: {}", e))?;

    for (file, name, kind, start_line, end_line, parent, visibility, is_impl) in &symbols {
        relations.add_symbol(file, name, kind, *start_line as u32);
        relations.add_symbol_range(file, name, *start_line as u32, *end_line as u32);
        relations.add_visibility(file, name, visibility);
        if let Some(parent_name) = parent {
            relations.add_parent(file, name, parent_name);
        }
        if *is_impl {
            relations.add_is_impl(file, name);
        }
    }

    // Get symbol attributes
    let attrs = idx
        .all_symbol_attributes()
        .await
        .map_err(|e| format!("Failed to get symbol attributes: {}", e))?;

    for (file, name, attribute) in &attrs {
        relations.add_attribute(file, name, attribute);
    }

    // Get symbol implements
    let implements = idx
        .all_symbol_implements()
        .await
        .map_err(|e| format!("Failed to get symbol implements: {}", e))?;

    for (file, name, interface) in &implements {
        relations.add_implements(file, name, interface);
    }

    // Get type methods
    let type_methods = idx
        .all_type_methods()
        .await
        .map_err(|e| format!("Failed to get type methods: {}", e))?;

    for (file, type_name, method_name) in &type_methods {
        relations.add_type_method(file, type_name, method_name);
    }

    // Get imports (file, module, name, line)
    let imports = idx
        .all_imports()
        .await
        .map_err(|e| format!("Failed to get imports: {}", e))?;

    for (file, module, name, _line) in imports {
        relations.add_import(&file, &module, &name);
    }

    // Get calls with qualifiers (caller_file, caller_symbol, callee_name, qualifier, line)
    let calls = idx
        .all_calls_with_qualifiers()
        .await
        .map_err(|e| format!("Failed to get calls: {}", e))?;

    for (file, caller, callee, qualifier, line) in &calls {
        relations.add_call(file, caller, callee, *line);
        if let Some(qual) = qualifier {
            relations.add_qualifier(file, caller, callee, qual);
        }
    }

    Ok(relations)
}

// =============================================================================
// Service-callable functions
// =============================================================================

use crate::service::facts::CommandResult;

/// Service-callable facts rules (compiled dylibs).
pub fn cmd_facts_rules_service(
    root: Option<&str>,
    rule: Option<&str>,
    pack: Option<&str>,
    list: bool,
) -> Result<CommandResult, String> {
    let root_path = root.map(PathBuf::from);
    let root_ref = root_path.as_deref();
    let pack_path = pack.map(PathBuf::from);
    let pack_ref = pack_path.as_deref();
    let rt = tokio::runtime::Runtime::new().map_err(|e| e.to_string())?;
    let exit_code = rt.block_on(cmd_rules(root_ref, rule, pack_ref, list, false));
    if exit_code == 0 {
        Ok(CommandResult {
            success: true,
            message: None,
            data: None,
        })
    } else {
        Err("Rule execution failed".to_string())
    }
}

/// Service-callable facts check (interpreted Datalog).
pub fn cmd_check_service(
    root: Option<&str>,
    rules_file: Option<&str>,
    list: bool,
) -> Result<CommandResult, String> {
    let effective_root = root
        .map(PathBuf::from)
        .map(Ok)
        .unwrap_or_else(std::env::current_dir)
        .map_err(|e| format!("Failed to get current directory: {e}"))?;
    let config = crate::config::NormalizeConfig::load(&effective_root);
    let rules_file_path = rules_file.map(PathBuf::from);
    let exit_code = super::rules::cmd_run_facts(
        &effective_root,
        rules_file_path.as_deref(),
        list,
        false,
        &config.analyze.facts_rules,
    );
    if exit_code == 0 {
        Ok(CommandResult {
            success: true,
            message: None,
            data: None,
        })
    } else {
        Err("Check failed".to_string())
    }
}
