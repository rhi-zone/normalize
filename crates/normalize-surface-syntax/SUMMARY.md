# normalize-surface-syntax

Surface-level syntax translation between programming languages via a common IR.

Parses source code from TypeScript, Lua, or Python into a minimal intermediate representation (`Program`, `Expr`, `Stmt`, `Function`) and emits equivalent code in a target language. Translation is syntax-mapping, not semantic transpilation — domain semantics remain with the runtime. Readers and writers are registered globally and looked up by language name or file extension via `reader_for_language`, `writer_for_language`. An optional `sexpr` feature serializes the IR to compact JSON S-expressions (`["std.let", "x", 1]`) used for storage in lotus verbs.
