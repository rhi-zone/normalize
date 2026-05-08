# xfile/rust fixture

Three-file Rust fixture for cross-file name resolution tests.
`utils.rs` defines utility functions (`add`, `multiply`), `models.rs` defines
`Calculator`, `main.rs` imports both. Tests verify the Rust `ModuleResolver`
produces correct `(module_path, resolution)` results for intra-workspace imports.
