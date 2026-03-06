# normalize-surface-syntax/src

Source for the surface-syntax translation crate.

Key modules: `ir/` (core IR types — `Program`, `Expr`, `Stmt`, `Function`, `BinaryOp`, `UnaryOp`), `input/` (language readers using tree-sitter), `output/` (language writers), `sexpr/` (S-expression serialization, feature-gated), `traits.rs` (`Reader` and `Writer` traits with `ReadError`), `registry.rs` (global reader/writer registry with `OnceLock`-based lazy init).
