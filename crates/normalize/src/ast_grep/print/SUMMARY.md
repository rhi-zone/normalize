# src/ast_grep/print

Output rendering for ast-grep match results. Defines the `PrintProcessor` and `Printer` traits for processing `NodeMatch` values into displayable output. Concrete implementations: `ColoredPrinter` (terminal-colored match display with context lines), `FileNamePrinter` (path-only output), `InteractivePrinter` (interactive rewrite confirmation), `JSONPrinter` (machine-readable match output). The `colored_print` submodule handles match merging and terminal styling.
