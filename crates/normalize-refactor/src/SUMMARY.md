# src/

Refactoring engine source.

- `lib.rs` — Core types (PlannedEdit, RefactoringPlan, RefactoringContext, References) and RefactoringExecutor
- `actions.rs` — Query and mutation primitives (locate symbol, find references, check conflicts, plan renames/deletes/inserts)
- `rename.rs` — Rename recipe: composes actions into a complete cross-file rename plan
- `extract.rs` — Extract-function recipe: lifts a byte-range selection into a new named function, infers free variables as parameters, inserts definition after enclosing function
