# src/commands/syntax

Command implementations for `normalize syntax` subcommands. Currently contains `node_types.rs` (`normalize syntax node-types`), which lists named kinds, anonymous kinds, and field names for a tree-sitter grammar. The AST (`ast.rs`) and query (`query.rs`) implementations remain in `commands/analyze/` since the syntax service references them there for now. Added 2026-03-15.
