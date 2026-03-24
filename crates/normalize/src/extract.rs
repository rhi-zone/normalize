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
        let index = self.index;
        let name = name.to_owned();
        let current_file = current_file.to_owned();

        let task = async move {
            // First try to resolve the import to find the source file
            if let Ok(Some((source_module, _original_name))) =
                index.resolve_import(&current_file, &name).await
            {
                let methods = index
                    .get_type_methods(&source_module, &name)
                    .await
                    .ok()
                    .unwrap_or_default();
                if !methods.is_empty() {
                    return Some(methods);
                }
            }

            // Also check if the type is defined in any indexed file
            if let Ok(files) = index.find_type_definitions(&name).await {
                for file in files {
                    if let Ok(methods) = index.get_type_methods(&file, &name).await
                        && !methods.is_empty()
                    {
                        return Some(methods);
                    }
                }
            }

            None
        };

        match tokio::runtime::Handle::try_current() {
            Ok(handle) => tokio::task::block_in_place(|| handle.block_on(task)),
            Err(_) => tokio::runtime::Runtime::new()
                .map_err(|e| tracing::warn!("failed to create runtime: {}", e))
                .ok()?
                .block_on(task),
        }
    }
}
