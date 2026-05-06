# src/

Refactoring engine source.

- `lib.rs` — Core types (PlannedEdit, RefactoringPlan, RefactoringContext, References) and RefactoringExecutor
- `actions.rs` — Query and mutation primitives (locate symbol, find references, check conflicts, plan renames/deletes/inserts). `decoration_extended_start` walks tree-sitter siblings to include leading doc comments / attributes / decorators when moving a symbol, classified by `node.kind()` rather than text
- `rename.rs` — Rename recipe: composes actions into a complete cross-file rename plan
- `move_item.rs` — Move recipe: relocates a symbol to a destination file (including leading doc comments / attributes / decorators identified via tree-sitter), deletes it from the source, and rewrites import statements in every file that imported it. Per-language module-path derivation is best-effort (Python, Go, JS/TS); unsupported languages emit warnings instead of generating wrong imports. Optional `--reexport` leaves a re-export stub at the source location (Python only)
- `inline_function.rs` — Inline-function recipe: locates a function definition and its (single) call site within the same file, substitutes arguments for parameters, replaces the call with the function body, and removes the definition. Supports JS/TS function declarations, arrow-function `const` bindings (`const f = (...) => ...`), Python `def`, and Rust `fn`. Conservative: aborts on multiple `return` statements or mismatched argument count. `--force` overrides the single-use check.
