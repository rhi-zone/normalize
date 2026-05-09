; F# CFG query
; Captures control flow nodes for CFG construction.
; See normalize-cfg for the full capture vocabulary.
; Verified against arborium F# grammar node types.
;
; F# is a functional-first language. Control flow includes if_expression,
; match_expression (via rule), for/while loops, and try expressions.

; ---------------------------------------------------------------------------
; if / elif / else (branch expression)
; ---------------------------------------------------------------------------

(if_expression
  condition: (_) @cfg.branch.condition
  then_expression: (_) @cfg.branch.then
  else_expression: (_) @cfg.branch.else
) @cfg.branch

(if_expression
  condition: (_) @cfg.branch.condition
  then_expression: (_) @cfg.branch.then
  .
  ; no else branch
) @cfg.branch

; ---------------------------------------------------------------------------
; match (pattern matching — rules as arms)
; ---------------------------------------------------------------------------

(match_expression
  expression: (_) @cfg.match.scrutinee
  (rule) @cfg.match.arm
) @cfg.match

; ---------------------------------------------------------------------------
; for (loop)
; ---------------------------------------------------------------------------

(for_expression
  ident: (_) @cfg.loop.condition
  to_expression: (_)
  do_expression: (_) @cfg.loop.body
) @cfg.loop

(for_each_expression
  pattern: (_) @cfg.loop.condition
  sequence_expression: (_)
  do_expression: (_) @cfg.loop.body
) @cfg.loop

; ---------------------------------------------------------------------------
; while (loop with condition)
; ---------------------------------------------------------------------------

(while_expression
  condition: (_) @cfg.loop.condition
  do_expression: (_) @cfg.loop.body
) @cfg.loop

; ---------------------------------------------------------------------------
; try / with / finally
; ---------------------------------------------------------------------------

(try_expression
  expression: (_) @cfg.try.body
) @cfg.try

(with_clause
  (rule) @cfg.try.catch
) @cfg.try.catch

(finally_clause) @cfg.try.finally

; ---------------------------------------------------------------------------
; Exits
; ---------------------------------------------------------------------------

(return_expression) @cfg.exit.return

(raise_expression) @cfg.exit.throw

(reraise) @cfg.exit.throw
