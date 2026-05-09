; Gleam CFG query
; Captures control flow nodes for CFG construction.
; See normalize-cfg for the full capture vocabulary.
; Verified against arborium Gleam grammar node types.
;
; Gleam is a functional language. Control flow is via case expressions
; and if expressions (Gleam has if since version 1.0).
; No loops, break, continue. panic!/todo! are exit forms.

; ---------------------------------------------------------------------------
; case (match/pattern matching)
; ---------------------------------------------------------------------------

(case
  subjects: (_) @cfg.match.scrutinee
  (case_clause) @cfg.match.arm
) @cfg.match

; ---------------------------------------------------------------------------
; if (boolean branch — Gleam 1.0+)
; ---------------------------------------------------------------------------

(if
  condition: (_) @cfg.branch.condition
  body: (_) @cfg.branch.then
  (else_clause) @cfg.branch.else
) @cfg.branch

(if
  condition: (_) @cfg.branch.condition
  body: (_) @cfg.branch.then
  .
) @cfg.branch

; ---------------------------------------------------------------------------
; Exits
; ---------------------------------------------------------------------------

; panic! and todo! are exit expressions in Gleam
(panic) @cfg.exit.throw

(todo) @cfg.exit.throw
