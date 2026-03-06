# normalize-surface-syntax/src/ir

Core IR types for surface-syntax translation.

`expr.rs` defines `Expr` (variable references, binary/unary operations, calls, member access, arrays, objects, anonymous functions, conditionals, assignments), `Literal`, `BinaryOp`, and `UnaryOp`. `stmt.rs` defines `Stmt` (variable declarations, expression statements, if/while/for, return, function definitions, try/catch). `mod.rs` defines `Program` (top-level body) and `Function`. `structure_eq.rs` provides `StructureEq` for comparing IR trees while ignoring surface hints like mutability or computed properties.
