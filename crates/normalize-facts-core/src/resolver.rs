//! Cross-file interface resolver trait.
//!
//! Defined here (in `normalize-facts-core`) so that the `Language` trait in
//! `normalize-languages` can reference it in `post_process_symbols` without
//! creating a dependency on `normalize-facts`.

/// Resolver for cross-file interface method lookups.
/// Used to find interface/class method signatures from other files.
pub trait InterfaceResolver: Send + Sync {
    /// Get method names for an interface/class by name.
    /// Returns None if the interface cannot be resolved (external, missing, etc.).
    fn resolve_interface_methods(&self, name: &str, current_file: &str) -> Option<Vec<String>>;
}
