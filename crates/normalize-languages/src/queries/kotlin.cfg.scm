; Kotlin CFG query
; Captures control flow nodes for CFG construction.
; See normalize-cfg for the full capture vocabulary.
; Verified against arborium Kotlin grammar node types.
;
; Note: Kotlin uses if_expression (not if_statement) and when_expression
; for conditional branching. when replaces switch.

; ---------------------------------------------------------------------------
; if / else (branch) — expression in Kotlin
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
; when (match) — replaces switch in Kotlin
; ---------------------------------------------------------------------------

(when_expression
  (when_subject) @cfg.match.scrutinee
  (when_entry) @cfg.match.arm
) @cfg.match

; when without subject (used as if-else chain)
(when_expression
  (when_entry) @cfg.match.arm
) @cfg.match

; ---------------------------------------------------------------------------
; for (loop over collection)
; ---------------------------------------------------------------------------

(for_statement
  (loop_range) @cfg.loop.condition
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

(do_while_statement
  body: (_) @cfg.loop.body
  condition: (_) @cfg.loop.condition
) @cfg.loop

; ---------------------------------------------------------------------------
; try / catch / finally
; ---------------------------------------------------------------------------

(try_expression
  block: (_) @cfg.try.body
) @cfg.try

(catch_block) @cfg.try.catch

(finally_block) @cfg.try.finally

; ---------------------------------------------------------------------------
; Exits
; ---------------------------------------------------------------------------

(return_at) @cfg.exit.return
(return) @cfg.exit.return

(break_at) @cfg.exit.break
(break) @cfg.exit.break

(continue_at) @cfg.exit.continue
(continue) @cfg.exit.continue

(throw) @cfg.exit.throw
