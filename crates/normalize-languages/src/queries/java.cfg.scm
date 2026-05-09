; Java CFG query
; Captures control flow nodes for CFG construction.
; See normalize-cfg for the full capture vocabulary.
; Verified against arborium-java node-types.json.
;
; Labeled break/continue: Java's `break label` and `continue label` are
; represented as `break_statement` / `continue_statement` nodes — the label
; is an optional child identifier. The CFG builder treats these as exits to
; the innermost enclosing loop; full label resolution is tracked in TODO.md.

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
; switch (match) — switch_expression covers both statement and expression form
; ---------------------------------------------------------------------------

(switch_expression
  condition: (_) @cfg.match.scrutinee
  body: (switch_block
    (switch_block_statement_group) @cfg.match.arm
  )
) @cfg.match

(switch_expression
  condition: (_) @cfg.match.scrutinee
  body: (switch_block
    (switch_rule) @cfg.match.arm
  )
) @cfg.match

; ---------------------------------------------------------------------------
; for (C-style loop)
; ---------------------------------------------------------------------------

(for_statement
  condition: (_) @cfg.loop.condition
  body: (_) @cfg.loop.body
) @cfg.loop

; for_statement without explicit condition (infinite loop)
(for_statement
  body: (_) @cfg.loop.body
) @cfg.loop

; ---------------------------------------------------------------------------
; enhanced for (for-each)
; ---------------------------------------------------------------------------

(enhanced_for_statement
  value: (_) @cfg.loop.condition
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
; try / catch / finally (including try-with-resources)
; ---------------------------------------------------------------------------

(try_statement
  body: (_) @cfg.try.body
) @cfg.try

(try_with_resources_statement
  body: (_) @cfg.try.body
) @cfg.try

(catch_clause) @cfg.try.catch

(finally_clause) @cfg.try.finally

; ---------------------------------------------------------------------------
; Exits
; ---------------------------------------------------------------------------

(return_statement) @cfg.exit.return

; break and continue — labeled or unlabeled; label resolution deferred
(break_statement) @cfg.exit.break

(continue_statement) @cfg.exit.continue

(throw_statement) @cfg.exit.throw
