# src/rg/flags

Ripgrep's complete flag definition and parsing layer, vendored from ripgrep 15.1.0. Defines all ripgrep flags as implementations of the `Flag` trait (in `defs.rs`), parses them into `LowArgs` then resolves to `HiArgs` (the high-level argument struct used by the search engine). Also responsible for generating shell completions and documentation. Submodules: `complete` (shell completion generators), `doc` (help text and man page generation), `hiargs`, `lowargs`, `parse`, `config`, `defs`.
