; Elixir scope analysis.
;
; Named functions (def/defp) don't have a distinct AST node — they're
; represented as call nodes with target "def"/"defp". Without predicate
; support we can't capture their parameters as definitions here.
;
; This file covers anonymous functions (fn...end), stab clauses,
; and pattern match bindings (x = expr).

; Scopes
; ------

[
  (anonymous_function)
  (stab_clause)
  (do_block)
] @local.scope

; Definitions
; -----------

; Anonymous function parameters: fn x, y -> ... end
; The stab_clause left: field holds the argument list.
(stab_clause
  left: (arguments
    (identifier) @local.definition))

; Pattern match binding: x = expr defines x.
;
; We can't use (#eq? @op "=") on the unnamed operator node because tree-sitter
; doesn't evaluate text predicates on unnamed node captures in field position.
; Instead, we capture the binary_operator node itself (@_binop) and apply a
; custom predicate (#is-match-op! @_binop) handled in the scope engine's Rust
; code, which walks the node's children to check for an unnamed "=" child.
((binary_operator
  left: (identifier) @local.definition) @_binop
 (#is-match-op! @_binop))

; References
; ----------

(identifier) @local.reference
