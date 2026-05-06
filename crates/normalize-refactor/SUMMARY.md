# normalize-refactor

Composable refactoring engine for normalize — domain logic extracted from the main crate.

Three layers:
- **Actions** (`src/actions.rs`): Pure query and mutation primitives
- **Recipes** (`src/rename.rs`, `src/move_item.rs`, `src/introduce_variable.rs`): Compositions of actions into complete refactoring plans
- **Executor** (`src/lib.rs` `RefactoringExecutor`): Shared apply/dry-run/shadow logic

Dependencies: normalize-edit, normalize-facts, normalize-languages, normalize-shadow.

`decoration_extended_start` in `actions.rs` uses `GrammarLoader::get_decorations()` to load language-specific `.scm` queries (`@decoration` captures) when available, falling back to the hardcoded `DECORATION_KINDS` list for languages without a `decorations.scm` file.

`introduce_variable` recipe (`src/introduce_variable.rs`): extracts an expression at a given byte range into a named variable binding. Parses the file with tree-sitter, walks up the CST to find the parent statement, inserts the binding before the statement, and replaces the expression with the variable name. Language-specific keyword: Python uses `name = expr`, JS/TS use `const name = expr;`, all others use `let name = expr;`. Exposed as `normalize edit introduce-variable <file> <range> <name>`.
