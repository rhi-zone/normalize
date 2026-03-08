//! Facts management commands (file index, symbols, calls, imports).

use crate::index;
use normalize_facts_rules_api::Relations;
use std::path::Path;

/// What to extract during indexing (files are always indexed).
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize, schemars::JsonSchema)]
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
