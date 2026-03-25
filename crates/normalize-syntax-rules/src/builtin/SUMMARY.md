# normalize-syntax-rules/src/builtin

Embedded builtin rule files, compiled into the binary via `include_str!`.

`mod.rs` declares the `BUILTIN_RULES` constant (a `&[BuiltinRule]` slice) referencing all 95 `.scm` rule files. Rules are organized by language namespace: Rust (14: `chained-if-let`, `commented-out-code`, `dbg-macro`, `expect-empty`, `magic-number`, `missing-module-doc`, `numeric-type-annotation`, `println-debug`, `static-mut`, `todo-macro`, `tuple-return`, `unnecessary-let`, `unnecessary-type-alias`, `unwrap-in-impl`), JavaScript (10), TypeScript (5), Python (12), Go (9), C/C++ (4), Java (6), C# (6), Kotlin (5), Swift (5), PHP (5), Ruby (9), and cross-language (4: `no-todo-comment`, `no-fixme-comment`, `hardcoded-secret`, `commented-out-code`). The `missing-module-doc` rule uses the `files` inclusion filter to restrict matching to `lib.rs` and `mod.rs` files only.
