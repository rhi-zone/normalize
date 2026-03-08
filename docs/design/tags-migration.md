# tags.scm Symbol Extraction Migration

## Status: COMPLETE

The migration from Language trait node-kind methods (`container_kinds`, `function_kinds`,
`type_kinds`, `public_symbol_kinds`, `extract_function`, `extract_container`, `extract_type`)
to tree-sitter `tags.scm` queries is **structurally complete**. These trait methods no longer
exist. The generic pipeline (`collect_symbols_from_tags`, `tags_capture_to_kind`,
`build_symbol_from_def`) is active and handles all symbol extraction.

## Current Architecture

```
Extractor::extract_with_support(content, support, resolver, file)
    │
    ├─ parsers::parse_with_grammar(grammar_name, content)  →  tree_sitter::Tree
    │
    ├─ loader.get_tags(grammar_name)  →  Option<Arc<String>>
    │     ├─ 68 bundled *.tags.scm files (compile-time include_str!)
    │     └─ external override files in search paths
    │
    ├─ if tags query present:
    │     collect_symbols_from_tags(tree, query, content, support, include_private)
    │         ├─ tags_capture_to_kind(capture_name)  →  Option<SymbolKind>
    │         ├─ TagDef { node, kind, is_method_capture, is_container, start/end_line }
    │         ├─ nesting reconstruction by line-range containment
    │         └─ build_symbol_from_def(def, content, support, in_container)
    │               ├─ support.node_name(node, content)
    │               ├─ support.refine_kind(node, content, tag_kind)
    │               ├─ support.build_signature(node, content)
    │               ├─ support.extract_docstring(node, content)
    │               ├─ support.extract_attributes(node, content)
    │               ├─ support.extract_implements(node, content) [containers only]
    │               └─ support.get_visibility(node, content)
    │
    └─ if no tags query: returns Vec::new()

    post-process:
    ├─ grammar == "rust":                merge_rust_impl_blocks(symbols)
    └─ grammar == "typescript"|"tsx":    mark_interface_implementations(symbols)
```

## Language Trait (current — already at target state)

**Required** (no default): `name()`, `extensions()`, `grammar_name()`

**Semantic hooks** (all have defaults, override selectively):
- `has_symbols()` — 7 languages return `false` (markup/data formats)
- `node_name()` — 20 languages override (non-standard `name` field)
- `refine_kind()` — 3 languages override (Rust, Go, TypeScript)
- `build_signature()` — 27 languages override
- `extract_docstring()`, `extract_attributes()`, `extract_implements()`, `get_visibility()`
- `extract_imports()`, `format_import()`, `signature_suffix()`
- `is_test_symbol()`, `test_file_globs()`, `embedded_content()`
- `container_body()`, `body_has_docstring()`, `analyze_container_body()`

## Cleanup Items — All Done

All four cleanup items tracked here are complete:

1. **`definition.var` mapped** — `tags_capture_to_kind` in `extract.rs` maps `"definition.var"` to `SymbolKind::Variable`.
2. **Stale "Previously in container/function/type_kinds" comments** — removed from all language files.
3. **Stale comment in `normalize-edit/src/lib.rs`** — fixed.
4. **Stale comment in `markdown.rs`** — fixed.

## What Does NOT Need to Change

- `has_symbols()` — still actively used in `ceremony.rs`, `docs.rs`, `search.rs`
- All semantic hooks listed above — called by the generic pipeline post-classification
- The `else { Vec::new() }` fallback in `extract_with_support` — correct for unknown languages
- `simple_symbol()`, `simple_function_symbol()` — pub exports, deprecate separately if desired
