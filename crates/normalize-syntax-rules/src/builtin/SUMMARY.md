# normalize-syntax-rules/src/builtin

Embedded builtin rule files, compiled into the binary via `include_str!`.

`mod.rs` declares the `BUILTIN_RULES` constant (a `&[BuiltinRule]` slice) that references all 94 `.scm` rule files. Rules are organized by language namespace: Rust (13), JavaScript (10), TypeScript (5), Python (12), Go (9), C/C++ (3+1), Java (6), C# (6: `console-write`, `empty-catch`, `goto`, `magic-number`, `thread-sleep`, `suppress-warnings`), Kotlin (5: `println-debug`, `empty-catch`, `magic-number`, `thread-sleep`, `suppress-warnings`), Swift (5: `print-debug`, `empty-catch`, `magic-number`, `force-unwrap`, `thread-sleep`), PHP (5: `debug-print`, `empty-catch`, `goto`, `magic-number`, `eval`), Ruby (9), and cross-language (4: `no-todo-comment`, `no-fixme-comment`, `hardcoded-secret`, `commented-out-code`). Updated 2026-03-13.
