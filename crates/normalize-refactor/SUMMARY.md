# normalize-refactor

Composable refactoring engine for normalize — domain logic extracted from the main crate.

Three layers:
- **Actions** (`src/actions.rs`): Pure query and mutation primitives
- **Recipes** (`src/rename.rs`, `src/move_item.rs`, `src/introduce_variable.rs`, `src/inline_variable.rs`, `src/add_parameter.rs`, `src/inline_function.rs`, `src/extract_function.rs`): Compositions of actions into complete refactoring plans
- **Executor** (`src/lib.rs` `RefactoringExecutor`): Shared apply/dry-run/shadow logic

Dependencies: normalize-edit, normalize-facts, normalize-languages, normalize-shadow.

`decoration_extended_start` in `actions.rs` uses `GrammarLoader::get_decorations()` to load language-specific `.scm` queries (`@decoration` captures) when available, falling back to the hardcoded `DECORATION_KINDS` list for languages without a `decorations.scm` file.

`introduce_variable` recipe (`src/introduce_variable.rs`): extracts an expression at a given byte range into a named variable binding. Parses the file with tree-sitter, walks up the CST to find the parent statement, inserts the binding before the statement, and replaces the expression with the variable name. Language-specific keyword: Python uses `name = expr`, JS/TS use `const name = expr;`, all others use `let name = expr;`. Exposed as `normalize edit introduce-variable <file> <range> <name>`.

`add_parameter` recipe (`src/add_parameter.rs`): inserts a new parameter into a function signature at a given 0-based position (default: last) and updates all call sites by inserting the default value at the same argument position. Parses files with tree-sitter to locate function parameter lists and call argument lists. Uses `actions::find_references` to find all callers via the facts index; falls back with a warning if the index is unavailable. Supports Rust (`function_item`/`parameters`/`arguments`), TypeScript/JavaScript (`function_declaration`/`formal_parameters`/`arguments`), and Python (`function_definition`/`parameters`/`argument_list`). Exposed as `normalize edit add-parameter <file> <function> --param <name> --default <value> [--type <type>] [--position <N>] [--dry-run]`.

`inline_function.rs` locates a function definition and its call site via tree-sitter traversal, substitutes arguments for parameters using whole-word replacement, and removes the definition. Supports function declarations, arrow-function `const` bindings, Python `def`, and Rust `fn`; conservative on multiple-return bodies.

`extract_function.rs` lifts a line range into a new function using CFG liveness data from the facts index (requires `normalize structure rebuild`). Queries `cfg_blocks`, `cfg_defs`, `cfg_uses`, `cfg_edges`, and `cfg_effects` tables; runs backward-dataflow liveness over the whole function to derive parameters and return values for the extracted region. Checks for async/generator/defer/acquire effects. Generates language-appropriate syntax for Rust, Python, Go, TypeScript/JavaScript, and Java. Dry-run by default; `--apply` writes the edit.

`tests/cross_file.rs` contains Phase 0 cross-file resolver integration tests: direct resolver unit tests for all 26 implemented languages, fixture-based tests under `tests/fixtures/xfile/`, and a `module_resolver_coverage_matrix` test asserting every supported language has a documented resolver status. `CallerRef` and `ImportRef` carry a `confidence` field (`"resolved"` | `"heuristic"`) set by `find_references` based on whether the language has a `ModuleResolver`.
