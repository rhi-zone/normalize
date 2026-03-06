# normalize-scope/src

Source for the scope analysis engine.

- `lib.rs` — the entire engine in one file: `ScopeEngine`, `find_references` implementation (scope stack walk, binding resolution), `Location`, `Reference`, `Definition`; handles `@local.scope`/`@local.definition`/`@local.reference` captures plus `@local.binding-leaf`/`@local.definition.each` extensions for recursive destructuring
