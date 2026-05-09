; C# CFG query
; Captures control flow nodes for CFG construction.
; See normalize-cfg for the full capture vocabulary.
; Verified against arborium C# grammar node types.

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
    (switch_section) @cfg.match.arm
  )
) @cfg.match

; switch expression (C# 8+)
(switch_expression
  value: (_) @cfg.match.scrutinee
  body: (switch_expression_body
    (switch_expression_arm) @cfg.match.arm
  )
) @cfg.match

; ---------------------------------------------------------------------------
; for (C-style loop)
; ---------------------------------------------------------------------------

(for_statement
  condition: (_) @cfg.loop.condition
  body: (_) @cfg.loop.body
) @cfg.loop

(for_statement
  body: (_) @cfg.loop.body
) @cfg.loop

; ---------------------------------------------------------------------------
; foreach (loop over collection)
; ---------------------------------------------------------------------------

(foreach_statement
  expression: (_) @cfg.loop.condition
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

; ---------------------------------------------------------------------------
; Exception type captures (Phase 4: type-refined exception flow)
; ---------------------------------------------------------------------------

; Thrown type: throw new InvalidOperationException() → captures "InvalidOperationException"
(throw_statement
  (object_creation_expression
    type: (identifier) @cfg.exit.throw.type))

; Catch type: catch (InvalidOperationException e) → captures "InvalidOperationException"
(catch_clause
  (catch_declaration
    type: (_) @cfg.try.catch.type))
