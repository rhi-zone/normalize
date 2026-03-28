# normalize-refactor

Composable refactoring engine for normalize — domain logic extracted from the main crate.

Three layers:
- **Actions** (`src/actions.rs`): Pure query and mutation primitives
- **Recipes** (`src/rename.rs`): Compositions of actions into complete refactoring plans
- **Executor** (`src/lib.rs` `RefactoringExecutor`): Shared apply/dry-run/shadow logic

Dependencies: normalize-edit, normalize-facts, normalize-languages, normalize-shadow.
