; Go CFG query
; Captures control flow nodes for CFG construction.
; See normalize-cfg for the full capture vocabulary.
; Note: verified against the arborium Go grammar tree-sitter node types.

; ---------------------------------------------------------------------------
; if (branch)
; ---------------------------------------------------------------------------

(if_statement
  consequence: (_) @cfg.branch.then
  alternative: (_) @cfg.branch.else
) @cfg.branch

(if_statement
  consequence: (_) @cfg.branch.then
  .
  ; no alternative
) @cfg.branch

; ---------------------------------------------------------------------------
; switch / expression_switch (match)
; ---------------------------------------------------------------------------

(expression_switch_statement
  value: (_) @cfg.match.scrutinee
  (expression_case_clause) @cfg.match.arm
) @cfg.match

(expression_switch_statement
  (expression_case_clause) @cfg.match.arm
) @cfg.match

; ---------------------------------------------------------------------------
; for (loop — covers for-range, for-condition, and unconditional)
; ---------------------------------------------------------------------------

(for_statement
  body: (_) @cfg.loop.body
) @cfg.loop

; ---------------------------------------------------------------------------
; Exits
; ---------------------------------------------------------------------------

(return_statement) @cfg.exit.return

(break_statement) @cfg.exit.break

(continue_statement) @cfg.exit.continue

; ---------------------------------------------------------------------------
; Def/use sites
; ---------------------------------------------------------------------------

; y := expr — short variable declaration
(short_var_declaration
  left: (expression_list
    (identifier) @cfg.def.name
  )
) @cfg.def

; y = expr — assignment
(assignment_statement
  left: (expression_list
    (identifier) @cfg.def.name
  )
) @cfg.def
