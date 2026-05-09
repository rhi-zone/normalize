; ReScript CFG query
; Captures control flow nodes for CFG construction.
; See normalize-cfg for the full capture vocabulary.
; Verified against arborium ReScript grammar node types.
;
; ReScript is a functional language compiling to JavaScript.
; Control flow: if expressions, switch expressions (pattern matching).

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
) @cfg.branch

; ---------------------------------------------------------------------------
; switch (match/pattern matching)
; ---------------------------------------------------------------------------

(switch_expression
  value: (_) @cfg.match.scrutinee
  (switch_match) @cfg.match.arm
) @cfg.match

; ---------------------------------------------------------------------------
; Exits
; ---------------------------------------------------------------------------

(return_expression) @cfg.exit.return

; raise is the throw equivalent
(call_expression
  function: (value_identifier) @_fn
  (#match? @_fn "^(raise|failwith|invalid_arg)$")
) @cfg.exit.throw
