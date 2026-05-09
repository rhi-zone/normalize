; VHDL CFG query
; Captures control flow nodes for CFG construction.
; See normalize-cfg for the full capture vocabulary.
; Verified against arborium VHDL grammar node types.

; ---------------------------------------------------------------------------
; if / elsif / else (branch)
; ---------------------------------------------------------------------------

(if_statement
  condition: (_) @cfg.branch.condition
  (sequence_of_statements) @cfg.branch.then
  (elsif_sequence_of_statements) @cfg.branch.else
) @cfg.branch

(if_statement
  condition: (_) @cfg.branch.condition
  (sequence_of_statements) @cfg.branch.then
  (else_sequence_of_statements
    (sequence_of_statements) @cfg.branch.else)
) @cfg.branch

(if_statement
  condition: (_) @cfg.branch.condition
  (sequence_of_statements) @cfg.branch.then
  .
) @cfg.branch

; ---------------------------------------------------------------------------
; case (match)
; ---------------------------------------------------------------------------

(case_statement
  expression: (_) @cfg.match.scrutinee
  (case_statement_alternative) @cfg.match.arm
) @cfg.match

; ---------------------------------------------------------------------------
; loop (various loop forms)
; ---------------------------------------------------------------------------

(loop_statement
  (iteration_scheme) @cfg.loop.condition
  (sequence_of_statements) @cfg.loop.body
) @cfg.loop

(loop_statement
  (sequence_of_statements) @cfg.loop.body
) @cfg.loop

; ---------------------------------------------------------------------------
; Exits
; ---------------------------------------------------------------------------

(return_statement) @cfg.exit.return

(exit_statement) @cfg.exit.break

(next_statement) @cfg.exit.continue
