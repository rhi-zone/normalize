; TSX CFG query
; Captures control flow nodes for CFG construction.
; See normalize-cfg for the full capture vocabulary.
; Verified against arborium-tsx node-types.json.
; TSX uses the same control flow grammar as TypeScript — node types are identical.

; ---------------------------------------------------------------------------
; if / else (branch)
; ---------------------------------------------------------------------------

(if_statement
  condition: (_) @cfg.branch.condition
  consequence: (_) @cfg.branch.then
  alternative: (_) @cfg.branch.else
) @cfg.branch

(if_statement
  condition: (_) @cfg.branch.condition
  consequence: (_) @cfg.branch.then
  .
  ; no alternative
) @cfg.branch

; ---------------------------------------------------------------------------
; switch (match)
; ---------------------------------------------------------------------------

(switch_statement
  value: (_) @cfg.match.scrutinee
  body: (switch_body
    (switch_case) @cfg.match.arm
  )
) @cfg.match

(switch_statement
  value: (_) @cfg.match.scrutinee
  body: (switch_body
    (switch_default) @cfg.match.arm
  )
) @cfg.match

; ---------------------------------------------------------------------------
; for / for-in / for-of (loop)
; ---------------------------------------------------------------------------

(for_statement
  condition: (_) @cfg.loop.condition
  body: (_) @cfg.loop.body
) @cfg.loop

(for_in_statement
  left: (_) @cfg.loop.condition
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
; do-while (loop with condition at end)
; ---------------------------------------------------------------------------

(do_statement
  body: (_) @cfg.loop.body
  condition: (_) @cfg.loop.condition
) @cfg.loop

; ---------------------------------------------------------------------------
; try / catch / finally
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

(break_statement) @cfg.exit.break

(continue_statement) @cfg.exit.continue

(throw_statement) @cfg.exit.throw
