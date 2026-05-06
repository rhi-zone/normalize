# normalize-surface-syntax/src/ir

Core IR types for surface-syntax translation.

`expr.rs` defines `Expr` (variable references, binary/unary operations, calls, member access, arrays, objects, anonymous functions, conditionals, assignments), `Literal`, `BinaryOp`, and `UnaryOp`. Structured variants (`Binary`, `Unary`, `Call`, `Member`, `Conditional`, `Assign`) carry an optional `span: Option<Span>` for source location tracking. `stmt.rs` defines `Stmt` (variable declarations, expression statements, if/while/for, return, function definitions, try/catch); structured variants also carry `span: Option<Span>`. `mod.rs` defines `Program` (top-level body), `Function`, and `Span { start_line, start_col, end_line, end_col }` (1-based lines, 0-based columns). `structure_eq.rs` provides `StructureEq` for comparing IR trees while ignoring surface hints like mutability, computed properties, and spans.
