# normalize-surface-syntax/src/sexpr

S-expression serialization for the surface-syntax IR (feature: `sexpr`).

Converts between the IR and a compact JSON array format where opcodes are namespaced strings: `["std.let", "x", 1]`, `["math.add", left, right]`, `["std.if", cond, then, else]`. `to_sexpr.rs` encodes `Program`/`Expr`/`Stmt` to `serde_json::Value` arrays. `from_sexpr.rs` decodes them back, reporting `SExprError` for unknown opcodes, wrong arity, or malformed input. This format is used for storage in lotus verbs.
