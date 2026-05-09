; Scala CFG query
; Captures control flow nodes for CFG construction.
; See normalize-cfg for the full capture vocabulary.
; Verified against arborium Scala grammar node types.
;
; Scala uses expression-oriented constructs: if_expression, match_expression,
; while_expression, for_expression. There is no switch — match covers that.

; ---------------------------------------------------------------------------
; if / else (branch) — expression in Scala
; ---------------------------------------------------------------------------

(if_expression
  condition: (_) @cfg.branch.condition
  consequence: (_) @cfg.branch.then
  alternative: (_) @cfg.branch.else
) @cfg.branch

(if_expression
  condition: (_) @cfg.branch.condition
  consequence: (_) @cfg.branch.then
  .
  ; no alternative
) @cfg.branch

; ---------------------------------------------------------------------------
; match (match expression)
; ---------------------------------------------------------------------------

(match_expression
  value: (_) @cfg.match.scrutinee
  body: (match_block
    (case_clause) @cfg.match.arm
  )
) @cfg.match

; ---------------------------------------------------------------------------
; for (for-comprehension / for loop)
; ---------------------------------------------------------------------------

(for_expression
  enumerators: (_) @cfg.loop.condition
  body: (_) @cfg.loop.body
) @cfg.loop

; ---------------------------------------------------------------------------
; while (loop with condition)
; ---------------------------------------------------------------------------

(while_expression
  condition: (_) @cfg.loop.condition
  body: (_) @cfg.loop.body
) @cfg.loop

; ---------------------------------------------------------------------------
; do-while (loop with condition at end)
; ---------------------------------------------------------------------------

(do_while_expression
  body: (_) @cfg.loop.body
  condition: (_) @cfg.loop.condition
) @cfg.loop

; ---------------------------------------------------------------------------
; try / catch / finally
; ---------------------------------------------------------------------------

(try_expression
  body: (_) @cfg.try.body
) @cfg.try

(catch_clause) @cfg.try.catch

(finally_clause) @cfg.try.finally

; ---------------------------------------------------------------------------
; Exits
; ---------------------------------------------------------------------------

(return_expression) @cfg.exit.return

(break_statement) @cfg.exit.break

(continue) @cfg.exit.continue

(throw_expression) @cfg.exit.throw
