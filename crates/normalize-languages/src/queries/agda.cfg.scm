; Agda CFG query
; Captures control flow nodes for CFG construction.
; See normalize-cfg for the full capture vocabulary.
; Verified against arborium Agda grammar node types.
;
; Agda is a dependently-typed proof assistant / functional language.
; Control flow is purely via pattern matching in function definitions
; and lambda clauses. There is no if-then-else syntax node in the grammar
; (it's encoded as pattern matching). No loops, breaks, or throws.

; ---------------------------------------------------------------------------
; Pattern matching in function definitions (branch-like)
; ---------------------------------------------------------------------------

; Each function clause with pattern matching is a branch
(function
  (lhs) @cfg.branch.condition
  (rhs) @cfg.branch.then
) @cfg.branch

; Lambda clause with pattern matching
(lambda_clause
  (lhs) @cfg.branch.condition
  (rhs) @cfg.branch.then
) @cfg.branch
