; Idris CFG query
; Captures control flow nodes for CFG construction.
; See normalize-cfg for the full capture vocabulary.
; Verified against arborium Idris grammar node types.
;
; Idris is a dependently-typed functional language.
; Control flow: if-then-else, case expressions.
; No loops or imperative exits.

; ---------------------------------------------------------------------------
; if / else (branch expression — exp_if)
; ---------------------------------------------------------------------------

(exp_if
  condition: (_) @cfg.branch.condition
  then: (_) @cfg.branch.then
  else: (_) @cfg.branch.else
) @cfg.branch

; ---------------------------------------------------------------------------
; case (match/pattern matching — exp_case)
; ---------------------------------------------------------------------------

(exp_case
  (exp) @cfg.match.scrutinee
  (alt) @cfg.match.arm
) @cfg.match
