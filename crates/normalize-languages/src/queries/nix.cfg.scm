; Nix CFG query
; Captures control flow nodes for CFG construction.
; See normalize-cfg for the full capture vocabulary.
; Verified against arborium Nix grammar node types.
;
; Nix is a purely functional language. The only control flow is
; if-then-else expressions. There are no loops, break, continue, or throw.

; ---------------------------------------------------------------------------
; if / else (branch expression)
; ---------------------------------------------------------------------------

(if_expression
  condition: (_) @cfg.branch.condition
  consequence: (_) @cfg.branch.then
  alternative: (_) @cfg.branch.else
) @cfg.branch
