; Elixir scope analysis.
;
; Named functions (def/defp) don't have a distinct AST node — they're
; represented as call nodes with target "def"/"defp". Without predicate
; support we can't capture their parameters as definitions here.
;
; This file covers anonymous functions (fn...end), stab clauses, and
; pattern match bindings (x = expr).

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

; Pattern match binding (x = expr) is intentionally omitted: tree-sitter
; text predicates (#eq?) don't work on unnamed node captures in field position,
; and the broad fallback (left of any binary_operator) produces false positives.

; References
; ----------

(identifier) @local.reference
