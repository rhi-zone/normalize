# src/ast_grep/print/colored_print

Supporting utilities for colored match output. `match_merger.rs` implements `MatchMerger`, which coalesces overlapping or adjacent AST matches that share context lines into a single display block. `styles.rs` defines terminal color styles (`PrintStyles`) for highlighting match regions. These are internal helpers used exclusively by `ColoredPrinter` in the parent module.
