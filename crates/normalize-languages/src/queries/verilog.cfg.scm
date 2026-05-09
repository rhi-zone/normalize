; Verilog/SystemVerilog CFG query
; Captures control flow nodes for CFG construction.
; See normalize-cfg for the full capture vocabulary.
; Verified against arborium Verilog grammar node types.

; ---------------------------------------------------------------------------
; if / else (branch — conditional_statement)
; ---------------------------------------------------------------------------

(conditional_statement
  cond_predicate: (_) @cfg.branch.condition
  statement_or_null: (_) @cfg.branch.then
  (else_clause
    statement_or_null: (_) @cfg.branch.else)
) @cfg.branch

(conditional_statement
  cond_predicate: (_) @cfg.branch.condition
  statement_or_null: (_) @cfg.branch.then
  .
) @cfg.branch

; ---------------------------------------------------------------------------
; case / casez / casex (match)
; ---------------------------------------------------------------------------

(case_statement
  case_expression: (_) @cfg.match.scrutinee
  (case_item) @cfg.match.arm
) @cfg.match

; ---------------------------------------------------------------------------
; for / foreach / forever / repeat (loop)
; ---------------------------------------------------------------------------

(loop_statement
  (for_initialization) @cfg.loop.condition
  (for_step) @cfg.loop.condition
  body: (_) @cfg.loop.body
) @cfg.loop

(loop_statement
  body: (_) @cfg.loop.body
) @cfg.loop

; ---------------------------------------------------------------------------
; Exits
; ---------------------------------------------------------------------------

(return_statement) @cfg.exit.return

(break_statement) @cfg.exit.break

(continue_statement) @cfg.exit.continue

(disable_statement) @cfg.exit.throw
