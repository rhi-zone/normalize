# normalize-surface-syntax

Surface-level syntax translation between programming languages via a common IR.

Parses source code from TypeScript, Lua, or Python into a minimal intermediate representation (`Program`, `Expr`, `Stmt`, `Function`) and emits equivalent code in a target language. Translation is syntax-mapping, not semantic transpilation — domain semantics remain with the runtime. Readers and writers are registered globally and looked up by language name or file extension via `reader_for_language`, `writer_for_language`. An optional `sexpr` feature serializes the IR to compact JSON S-expressions (`["std.let", "x", 1]`) used for storage in lotus verbs.

Tree-sitter grammars are loaded dynamically via `normalize_languages::parsers::grammar_loader()` — the same dlopen-based singleton used elsewhere in the workspace. No `arborium-*` crates are linked statically; all grammars come from the shared `~/.config/normalize/grammars/` install (or `NORMALIZE_GRAMMAR_PATH`).

The IR includes first-class nodes for: `Stmt::Import { source, names }` and `Stmt::Export { names, source }` for module imports/exports (TypeScript and Python readers populate these; Lua has no native import syntax); `Stmt::Class { name, extends, methods }` with `Method { name, params, body, is_static }` for class definitions (TypeScript and Python populate these; Lua writer lowers to metatable pattern). `Stmt::Comment { text, block }` preserves documentation comments: TypeScript readers capture `//`, `/* */`, and `/** JSDoc */`; Lua readers capture `--`, `---`, and `--[[ ]]`; writers emit in the target language's comment syntax. Source location spans (`Span`) are attached to all structured nodes by readers and ignored by writers.
