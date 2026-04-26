# normalize-refactor

Composable refactoring engine for normalize — domain logic extracted from the main crate.

Three layers:
- **Actions** (`src/actions.rs`): Pure query and mutation primitives
- **Recipes** (`src/rename.rs`, `src/move_item.rs`): Compositions of actions into complete refactoring plans
- **Executor** (`src/lib.rs` `RefactoringExecutor`): Shared apply/dry-run/shadow logic

Dependencies: normalize-edit, normalize-facts, normalize-languages, normalize-shadow.

`decoration_extended_start` in `actions.rs` uses `GrammarLoader::get_decorations()` to load language-specific `.scm` queries (`@decoration` captures) when available, falling back to the hardcoded `DECORATION_KINDS` list for languages without a `decorations.scm` file.
