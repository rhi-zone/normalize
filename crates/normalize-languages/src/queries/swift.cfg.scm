; Swift CFG query
; Captures control flow nodes for CFG construction.
; See normalize-cfg for the full capture vocabulary.
; Verified against arborium Swift grammar node types.

; ---------------------------------------------------------------------------
; if / guard / else (branch)
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

; guard (early exit — condition must be true to continue)
(guard_statement
  condition: (_) @cfg.branch.condition
  body: (_) @cfg.branch.then
) @cfg.branch

; ---------------------------------------------------------------------------
; switch (match)
; ---------------------------------------------------------------------------

(switch_statement
  expr: (_) @cfg.match.scrutinee
  (switch_entry) @cfg.match.arm
) @cfg.match

; ---------------------------------------------------------------------------
; for-in (loop)
; ---------------------------------------------------------------------------

(for_statement
  (for_in_sequence) @cfg.loop.condition
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
; repeat-while (do-while equivalent in Swift)
; ---------------------------------------------------------------------------

(repeat_while_statement
  body: (_) @cfg.loop.body
  condition: (_) @cfg.loop.condition
) @cfg.loop

; ---------------------------------------------------------------------------
; do / catch (exception handling)
; ---------------------------------------------------------------------------

(do_statement
  body: (_) @cfg.try.body
) @cfg.try

(catch_block) @cfg.try.catch

; ---------------------------------------------------------------------------
; Exits
; ---------------------------------------------------------------------------

(return_statement) @cfg.exit.return

(break_statement) @cfg.exit.break

(continue_statement) @cfg.exit.continue

(throw_statement) @cfg.exit.throw
