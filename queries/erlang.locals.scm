; Erlang uses uppercase-starting identifiers for variables (var nodes).
; Atoms are lowercase and used for function names.

; Scopes
; ------

[
  (function_clause)
  (fun_clause)
  (cr_clause)
] @local.scope

; Definitions
; -----------

; Function names (atom in function_clause head)
(function_clause
  name: (atom) @local.definition)

; Function clause parameters: foo(X, Y) -> ...
; Top-level var nodes in the argument list are pattern-bound variables.
(function_clause
  args: (expr_args
    (var) @local.definition))

; Anonymous function clause parameters: fun(X, Y) -> ...
(fun_clause
  args: (expr_args
    (var) @local.definition))

; Case and receive clause patterns: case Expr of Pattern -> Body end
(cr_clause
  pat: (var) @local.definition)

; References
; ----------

(var) @local.reference
