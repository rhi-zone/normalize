; Fish shell CFG query
; Captures control flow nodes for CFG construction.
; See normalize-cfg for the full capture vocabulary.
; Verified against arborium Fish grammar node types.

; ---------------------------------------------------------------------------
; if / else if / else (branch)
; ---------------------------------------------------------------------------

(if_statement
  condition: (_) @cfg.branch.condition
  body: (_) @cfg.branch.then
  (else_clause
    body: (_) @cfg.branch.else)
) @cfg.branch

(if_statement
  condition: (_) @cfg.branch.condition
  body: (_) @cfg.branch.then
  (else_if_clause) @cfg.branch.else
) @cfg.branch

(if_statement
  condition: (_) @cfg.branch.condition
  body: (_) @cfg.branch.then
  .
) @cfg.branch

; ---------------------------------------------------------------------------
; switch / case (match)
; ---------------------------------------------------------------------------

(switch_statement
  value: (_) @cfg.match.scrutinee
  (case_clause) @cfg.match.arm
) @cfg.match

; ---------------------------------------------------------------------------
; for (loop)
; ---------------------------------------------------------------------------

(for_statement
  variable: (_) @cfg.loop.condition
  body: (_) @cfg.loop.body
) @cfg.loop

; ---------------------------------------------------------------------------
; while (loop with condition)
; ---------------------------------------------------------------------------

(while_statement
  condition: (_) @cfg.loop.condition
  body: (_) @cfg.loop.body
) @cfg.loop

; ---------------------------------------------------------------------------
; Exits
; ---------------------------------------------------------------------------

(return) @cfg.exit.return

(break) @cfg.exit.break

(continue) @cfg.exit.continue
