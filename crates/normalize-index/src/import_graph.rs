//! Import graph construction from the file index.
//!
//! Lives here (not in `normalize-architecture`) so that both `normalize-graph`
//! consumers and `normalize-architecture` can build the import graph without a
//! `graph ↔ architecture` dependency cycle. `normalize-architecture` re-exports
//! [`build_import_graph`] and [`ImportGraph`] for backwards compatibility.

use normalize_facts::FileIndex;
use std::collections::{HashMap, HashSet};

/// Import graph: maps of who imports whom and who is imported by whom.
pub struct ImportGraph {
    pub imports_by_file: HashMap<String, HashSet<String>>,
    pub importers_by_file: HashMap<String, HashSet<String>>,
    pub raw_import_count: usize,
}

/// Build an import graph from the index.
pub async fn build_import_graph(idx: &FileIndex) -> Result<ImportGraph, libsql::Error> {
    let mut imports_by_file: HashMap<String, HashSet<String>> = HashMap::new();
    let mut importers_by_file: HashMap<String, HashSet<String>> = HashMap::new();
    let mut unresolved = 0usize;

    let conn = idx.connection();
    let stmt = conn
        .prepare("SELECT file, module, name FROM imports")
        .await?;
    let mut rows = stmt.query(()).await?;

    let mut raw_imports: Vec<(String, String)> = Vec::new();
    while let Some(row) = rows.next().await? {
        let file: String = row.get(0)?;
        let module: Option<String> = row.get(1)?;
        let name: String = row.get(2)?;

        let full_module = match module {
            Some(m) if !m.is_empty() => {
                if m.contains("::") {
                    m
                } else if m == "crate" || m == "super" || m == "self" {
                    format!("{}::{}", m, name)
                } else {
                    m
                }
            }
            _ => {
                if let Some(pos) = name.rfind("::") {
                    name[..pos].to_string()
                } else {
                    continue;
                }
            }
        };

        raw_imports.push((file, full_module));
    }

    for (source_file, module) in &raw_imports {
        let resolved_files = idx.module_to_files(module, source_file).await;

        if resolved_files.is_empty() {
            unresolved += 1;
            continue;
        }

        for target_file in resolved_files {
            imports_by_file
                .entry(source_file.clone())
                .or_default()
                .insert(target_file.clone());
            importers_by_file
                .entry(target_file)
                .or_default()
                .insert(source_file.clone());
        }
    }

    let _ = if raw_imports.is_empty() {
        0.0
    } else {
        let resolved = raw_imports.len() - unresolved;
        (resolved as f64 / raw_imports.len() as f64) * 100.0
    };

    Ok(ImportGraph {
        imports_by_file,
        importers_by_file,
        raw_import_count: raw_imports.len(),
    })
}
