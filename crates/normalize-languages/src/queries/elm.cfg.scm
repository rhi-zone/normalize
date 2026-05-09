; Elm CFG query
; Captures control flow nodes for CFG construction.
; See normalize-cfg for the full capture vocabulary.
; Verified against arborium Elm grammar node types.
;
; Elm is a purely functional language. Control flow is via if-else
; expressions and case-of expressions (pattern matching).
; There are no loops, break, continue, or throw.

; ---------------------------------------------------------------------------
; if / else (branch expression)
; ---------------------------------------------------------------------------

(if_else_expr
  (expr) @cfg.branch.condition
  (expr) @cfg.branch.then
  (expr) @cfg.branch.else
) @cfg.branch

; ---------------------------------------------------------------------------
; case / of (match)
; ---------------------------------------------------------------------------

(case_of_expr
  (expr) @cfg.match.scrutinee
  (case_of_branch) @cfg.match.arm
) @cfg.match
