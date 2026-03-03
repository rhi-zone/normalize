; Elixir scope analysis.
;
; Named functions (def/defp) don't have a distinct AST node — they're
; represented as call nodes with target "def"/"defp". Without predicate
; support we can't capture their parameters as definitions here.
;
; This file covers anonymous functions (fn...end) and stab clauses,
; which do use explicit stab_clause nodes.

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

; References
; ----------

(identifier) @local.reference
