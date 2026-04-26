# src/

Refactoring engine source.

- `lib.rs` — Core types (PlannedEdit, RefactoringPlan, RefactoringContext, References) and RefactoringExecutor
- `actions.rs` — Query and mutation primitives (locate symbol, find references, check conflicts, plan renames/deletes/inserts)
- `rename.rs` — Rename recipe: composes actions into a complete cross-file rename plan
- `move_item.rs` — Move recipe: relocates a symbol to a destination file, deletes it from the source, and rewrites import statements in every file that imported it. Per-language module-path derivation is best-effort (Python, Go, JS/TS); unsupported languages emit warnings instead of generating wrong imports. Optional `--reexport` leaves a re-export stub at the source location (Python only)
