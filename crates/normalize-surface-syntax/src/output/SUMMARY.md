# normalize-surface-syntax/src/output

Language writers: emit surface-syntax IR as source code in a target language.

Contains writers for Lua (`lua.rs`), TypeScript (`typescript.rs`), JavaScript (`javascript.rs`), and Python (`python.rs`), each feature-gated (`write-lua`, `write-typescript`, `write-javascript`, `write-python`). Each writer implements the `Writer` trait and is registered as a static in the global registry. `mod.rs` re-exports the public writer types (`LuaWriter`, etc.)

The JavaScript writer delegates directly to the TypeScript writer, since the IR is emitted identically in both languages (TypeScript output is already valid JavaScript).

Type annotation output: TypeScript and Python writers emit type annotations from `Param::type_annotation`, `Function::return_type`, and `Stmt::Let::type_annotation`. TypeScript uses `: type` syntax on params and variables, `): return_type {` on functions. Python uses `: type` on params and `-> return_type` on functions. Lua ignores type annotations entirely.

Template literal output: TypeScript/JavaScript writers emit `TemplateLiteral` as backtick syntax `` `text${expr}` ``. Python writer emits as f-strings (`f"text{expr}"`). Lua writer lowers to string concatenation (`"text" .. expr`).

All writers handle `Stmt::Comment`: TypeScript/JavaScript emit `// line` or `/* block */` (JSDoc multi-line `/** ... */` when block text spans multiple lines or starts with `*`); Lua emits `-- line` or `--[[ block ]]`; Python emits `# line` or `"""block"""`.

