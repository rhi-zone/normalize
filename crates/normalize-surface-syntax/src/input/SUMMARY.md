# normalize-surface-syntax/src/input

Language readers: parse source code into the surface-syntax IR.

Contains tree-sitter-based readers for TypeScript (`typescript.rs`), Lua (`lua.rs`), and Python (`python.rs`), each feature-gated (`read-typescript`, `read-lua`, `read-python`). `mod.rs` re-exports the public entry points. Each reader implements the `Reader` trait and is registered as a static in the global registry.
