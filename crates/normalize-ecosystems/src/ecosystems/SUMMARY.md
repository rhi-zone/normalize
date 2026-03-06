# normalize-ecosystems/src/ecosystems

Ecosystem trait implementations and the global plugin registry.

`mod.rs` manages a `RwLock<Vec<&'static dyn Ecosystem>>` registry initialized with built-in ecosystems on first use and provides `detect_ecosystem`, `detect_all_ecosystems`, `get_ecosystem`, `list_ecosystems`, `all_ecosystems`, and `register`. Each sibling module (`cargo.rs`, `npm/`, `python.rs`, etc.) implements the `Ecosystem` trait for one package manager, including manifest detection, lockfile parsing, registry queries, dependency listing, and security audit.
