# normalize-languages/src

Source for the normalize-languages crate.

One file per language (e.g., `python.rs`, `rust.rs`, `go.rs`, `typescript.rs`, ...) plus shared infrastructure: `traits.rs` (`Language` trait, `ContainerBody`, `EmbeddedBlock`, `Symbol`, `Import`, `Visibility`, `simple_symbol`, `simple_function_symbol`), `grammar_loader.rs` (`GrammarLoader` — dynamic `.so`/`.dylib` loading with ABI version checking), `registry.rs` (global language registry), `parsers.rs` (global `GrammarLoader` singleton), `body.rs` (shared container-body utilities), `ecmascript.rs` (shared JS/TS extraction logic), `component.rs` (Vue/Svelte component support), `ast_grep.rs` (ast-grep integration), `ffi.rs` (C FFI helpers), `external_packages.rs` (external package index), and `queries/` (310 tree-sitter `.scm` query files).
