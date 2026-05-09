; Python CFG query
; Captures control flow nodes for CFG construction.
; See normalize-cfg for the full capture vocabulary.

; ---------------------------------------------------------------------------
; if / elif / else (branch)
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
; match (Python 3.10+)
; ---------------------------------------------------------------------------

(match_statement
  subject: (_) @cfg.match.scrutinee
  body: (block
    (case_clause) @cfg.match.arm
  )
) @cfg.match

; ---------------------------------------------------------------------------
; for (loop)
; ---------------------------------------------------------------------------

(for_statement
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
; try / except / finally
; ---------------------------------------------------------------------------

(try_statement
  body: (_) @cfg.try.body
) @cfg.try

(except_clause) @cfg.try.catch

(finally_clause) @cfg.try.finally

; ---------------------------------------------------------------------------
; Exits
; ---------------------------------------------------------------------------

(return_statement) @cfg.exit.return

(break_statement) @cfg.exit.break

(continue_statement) @cfg.exit.continue

(raise_statement) @cfg.exit.throw

; ---------------------------------------------------------------------------
; Def/use sites
; ---------------------------------------------------------------------------

; y = expr — assignment definition
(assignment
  left: (identifier) @cfg.def.name
) @cfg.def

; y: type = expr — annotated assignment definition
(augmented_assignment
  left: (identifier) @cfg.def.name
) @cfg.def
