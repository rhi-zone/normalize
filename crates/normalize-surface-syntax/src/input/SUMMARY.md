# normalize-surface-syntax/src/input

Language readers: parse source code into the surface-syntax IR.

Contains tree-sitter-based readers for TypeScript (`typescript.rs`), JavaScript (`javascript.rs`), Lua (`lua.rs`), and Python (`python.rs`), each feature-gated (`read-typescript`, `read-javascript`, `read-lua`, `read-python`). `mod.rs` re-exports the public entry points. Each reader implements the `Reader` trait and is registered as a static in the global registry.

The JavaScript reader delegates to the TypeScript reader's shared `ReadContext` logic via `read_with_language`, since both grammars use identical node kinds for all JavaScript constructs. TypeScript-only nodes (`interface_declaration`, `type_annotation`, etc.) are simply absent from JavaScript sources.

Comment preservation: TypeScript and JavaScript readers convert `comment` nodes to `Stmt::Comment` (line comments with `//` stripped, block comments with `/* */` or `/** */` stripped). Lua reader converts `comment` nodes to `Stmt::Comment` (line `--`, LuaDoc `---`, block `--[[ ]]`). Comments are emitted in the target language's syntax by the writer.
