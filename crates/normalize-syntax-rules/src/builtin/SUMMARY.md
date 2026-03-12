# normalize-syntax-rules/src/builtin

Embedded builtin rule files, compiled into the binary via `include_str!`.

`mod.rs` declares the `BUILTIN_RULES` constant (a `&[BuiltinRule]` slice) that references all 73 `.scm` rule files. Rules are organized by language namespace: Rust (13), JavaScript (10), TypeScript (5), Python (12), Go (9), C/C++ (3+1: `printf-debug`, `goto`, `magic-number` for C; `cout-debug` for C++), Java (6: `system-print`, `empty-catch`, `print-stack-trace`, `magic-number`, `suppress-warnings`, `thread-sleep`), Ruby (9), and cross-language (4: `no-todo-comment`, `no-fixme-comment`, `hardcoded-secret`, `commented-out-code`). Updated 2026-03-13.
