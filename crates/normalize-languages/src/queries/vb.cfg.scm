; Visual Basic .NET CFG query
; Captures control flow nodes for CFG construction.
; See normalize-cfg for the full capture vocabulary.
; Verified against arborium VB.NET grammar node types.

; ---------------------------------------------------------------------------
; If / Else (branch)
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
  .
  ; no else clause
) @cfg.branch

; ---------------------------------------------------------------------------
; Select Case (match)
; ---------------------------------------------------------------------------

(select_case_statement
  expression: (_) @cfg.match.scrutinee
  (case_clause) @cfg.match.arm
) @cfg.match

; ---------------------------------------------------------------------------
; For / For Each (loop)
; ---------------------------------------------------------------------------

(for_statement
  (for_to_clause
    from: (_) @cfg.loop.condition)
  body: (_) @cfg.loop.body
) @cfg.loop

(for_each_statement
  expression: (_) @cfg.loop.condition
  body: (_) @cfg.loop.body
) @cfg.loop

; ---------------------------------------------------------------------------
; While / Do (loop)
; ---------------------------------------------------------------------------

(while_statement
  condition: (_) @cfg.loop.condition
  body: (_) @cfg.loop.body
) @cfg.loop

(do_loop_statement
  body: (_) @cfg.loop.body
) @cfg.loop

; ---------------------------------------------------------------------------
; Try / Catch / Finally
; ---------------------------------------------------------------------------

(try_statement
  body: (_) @cfg.try.body
) @cfg.try

(catch_clause) @cfg.try.catch

(finally_clause) @cfg.try.finally

; ---------------------------------------------------------------------------
; Exits
; ---------------------------------------------------------------------------

(return_statement) @cfg.exit.return

(exit_statement) @cfg.exit.break

(continue_statement) @cfg.exit.continue

(throw_statement) @cfg.exit.throw
