# src/bin

Additional binary targets in the normalize crate. Currently contains a single binary: `debug_ast` — a development utility that prints the full tree-sitter concrete syntax tree for a code snippet given a grammar name and source string on the command line. Used when implementing or debugging language support to inspect node kinds and tree structure without running the full normalize CLI.
