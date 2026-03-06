# normalize-surface-syntax/src/output

Language writers: emit surface-syntax IR as source code in a target language.

Contains writers for Lua (`lua.rs`), TypeScript (`typescript.rs`), and Python (`python.rs`), each feature-gated (`write-lua`, `write-typescript`, `write-python`). Each writer implements the `Writer` trait and is registered as a static in the global registry. `mod.rs` re-exports the public writer types (`LuaWriter`, etc.).
