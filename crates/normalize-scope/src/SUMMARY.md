# normalize-scope/src

Source for the scope analysis engine.

- `lib.rs` — the entire engine in one file: `ScopeEngine`, `find_references` implementation (scope stack walk, binding resolution), `Location`, `Reference`, `Definition` (now with `kind: Option<String>` for capture subtypes); handles `@local.scope`/`@local.definition`/`@local.definition.<subkind>`/`@local.reference` captures plus `@local.binding-leaf`/`@local.definition.each` extensions for recursive destructuring; `find_unused_parameters()` returns parameters with `kind == "parameter"` that have no resolved reference
