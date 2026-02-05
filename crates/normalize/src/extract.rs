//! Shared symbol extraction from source code.
//!
//! This module re-exports the core extraction logic from `normalize_facts`
//! and adds `IndexedResolver` which depends on `FileIndex`.

// Re-export everything from normalize_facts::extract
pub use normalize_facts::{
    ExtractOptions, ExtractResult, Extractor, InterfaceResolver, OnDemandResolver,
};

// Also re-export compute_complexity
pub use normalize_facts::extract::compute_complexity;

/// Resolver that uses FileIndex for cross-file interface lookups.
/// This is the fast path when an index is available.
pub struct IndexedResolver<'a> {
    index: &'a crate::index::FileIndex,
}

impl<'a> IndexedResolver<'a> {
    pub fn new(index: &'a crate::index::FileIndex) -> Self {
        Self { index }
    }
}

impl InterfaceResolver for IndexedResolver<'_> {
    fn resolve_interface_methods(&self, name: &str, current_file: &str) -> Option<Vec<String>> {
        let rt = tokio::runtime::Runtime::new().ok()?;

        // First try to resolve the import to find the source file
        if let Ok(Some((source_module, _original_name))) =
            rt.block_on(self.index.resolve_import(current_file, name))
        {
            // Convert module to file path and query type_methods
            // For now, try the source_module as a relative path
            let methods = rt
                .block_on(self.index.get_type_methods(&source_module, name))
                .ok()?;
            if !methods.is_empty() {
                return Some(methods);
            }
        }

        // Also check if the type is defined in any indexed file
        if let Ok(files) = rt.block_on(self.index.find_type_definitions(name)) {
            for file in files {
                if let Ok(methods) = rt.block_on(self.index.get_type_methods(&file, name))
                    && !methods.is_empty()
                {
                    return Some(methods);
                }
            }
        }

        None
    }
}
