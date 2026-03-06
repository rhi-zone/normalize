# normalize-core

Foundational traits for the normalize ecosystem, shared across all sub-crates.

Exports the `Merge` trait (for layering global config with project config, where "other" wins) and re-exports the `#[derive(Merge)]` proc macro from `normalize-derive`. `Merge` is implemented for all Rust primitives, `String`, `PathBuf`, `Option<T>`, `Vec`, `HashMap`, `BTreeMap`, `HashSet`, and `BTreeSet`. Derive support is available for named-field structs.
