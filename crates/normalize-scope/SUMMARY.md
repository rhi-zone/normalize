# normalize-scope

Scope analysis engine using tree-sitter `locals.scm` queries — resolves symbol references to their definitions within a single source file for rename and find-references operations.

Key type: `ScopeEngine` (constructed from a `GrammarLoader`, exposes `find_references`). Output types: `Reference` (with optional resolved `Definition`), `Definition`, `Location`. Supports custom extension captures `@local.binding-leaf` and `@local.definition.each` for recursive destructuring patterns (JS/TS/TSX). All knowledge of which node kinds are bindings lives in `.scm` files — the engine has no hardcoded language rules. Single-module crate (all logic in `lib.rs`). Published as a standalone crate on crates.io (part of the 38-crate normalize workspace).
