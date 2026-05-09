; OCaml CFG query
; Captures control flow nodes for CFG construction.
; See normalize-cfg for the full capture vocabulary.
; Verified against arborium OCaml grammar node types.
;
; OCaml is expression-oriented. Control flow includes if_expression,
; match_expression, for_expression, while_expression.

; ---------------------------------------------------------------------------
; if / else (branch expression)
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
  ; no else branch
) @cfg.branch

; ---------------------------------------------------------------------------
; match (pattern matching)
; ---------------------------------------------------------------------------

(match_expression
  value: (_) @cfg.match.scrutinee
  (match_case) @cfg.match.arm
) @cfg.match

; ---------------------------------------------------------------------------
; for (counted loop)
; ---------------------------------------------------------------------------

(for_expression
  index: (_) @cfg.loop.condition
  first: (_)
  last: (_)
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
; try / with (exception handling)
; ---------------------------------------------------------------------------

(try_expression
  body: (_) @cfg.try.body
  (match_case) @cfg.try.catch
) @cfg.try

; ---------------------------------------------------------------------------
; Exits
; ---------------------------------------------------------------------------

; raise is the throw equivalent in OCaml
(application_expression
  function: (value_path (value_name) @_fn)
  (#eq? @_fn "raise")
) @cfg.exit.throw
