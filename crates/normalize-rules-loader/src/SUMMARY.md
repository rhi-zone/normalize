# normalize-rules-loader/src

Single-file source for the `normalize-rules-loader` crate.

`lib.rs` implements dylib discovery (`search_paths` builds the three search directories, `discover` filters by platform prefix/extension), `load_from_path` (calls `RulePackRef::load_from_file` from the `abi_stable`-based API), and `format_diagnostic` (formats a `Diagnostic` with optional ANSI color, handling `ROption` locations and suggestions from the ABI-stable types).
