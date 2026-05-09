; HCL (HashiCorp Configuration Language) CFG query
; Captures control flow nodes for CFG construction.
; See normalize-cfg for the full capture vocabulary.
; Verified against arborium HCL grammar node types.
;
; HCL has limited control flow: conditional (ternary) expressions
; and for expressions/comprehensions. No loops or exit statements.

; ---------------------------------------------------------------------------
; conditional (ternary expression — branch)
; ---------------------------------------------------------------------------

(conditional
  condition: (_) @cfg.branch.condition
  consequence: (_) @cfg.branch.then
  alternative: (_) @cfg.branch.else
) @cfg.branch

; ---------------------------------------------------------------------------
; for (comprehension — loop-like construct)
; ---------------------------------------------------------------------------

(for_tuple_expr
  (for_cond) @cfg.loop.condition
) @cfg.loop

(for_tuple_expr) @cfg.loop

(for_object_expr
  (for_cond) @cfg.loop.condition
) @cfg.loop

(for_object_expr) @cfg.loop
