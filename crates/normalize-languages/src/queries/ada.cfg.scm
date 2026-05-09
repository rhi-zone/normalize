; Ada CFG query
; Captures control flow nodes for CFG construction.
; See normalize-cfg for the full capture vocabulary.
; Verified against arborium Ada grammar node types.

; ---------------------------------------------------------------------------
; if / elsif / else (branch)
; ---------------------------------------------------------------------------

(if_statement
  condition: (_) @cfg.branch.condition
  statements: (_) @cfg.branch.then
  (elsif_statement_item) @cfg.branch.else
) @cfg.branch

(if_statement
  condition: (_) @cfg.branch.condition
  statements: (_) @cfg.branch.then
  (else_statement_item) @cfg.branch.else
) @cfg.branch

(if_statement
  condition: (_) @cfg.branch.condition
  statements: (_) @cfg.branch.then
  .
) @cfg.branch

; if expression (Ada 2012+)
(if_expression) @cfg.branch

; ---------------------------------------------------------------------------
; case (match)
; ---------------------------------------------------------------------------

(case_statement
  expression: (_) @cfg.match.scrutinee
  (case_statement_alternative) @cfg.match.arm
) @cfg.match

(case_expression
  expression: (_) @cfg.match.scrutinee
  (case_expression_alternative) @cfg.match.arm
) @cfg.match

; ---------------------------------------------------------------------------
; loop (various loop forms)
; ---------------------------------------------------------------------------

(loop_statement
  (loop_parameter_specification) @cfg.loop.condition
  (sequence_of_statements) @cfg.loop.body
) @cfg.loop

(loop_statement
  (iteration_scheme) @cfg.loop.condition
  (sequence_of_statements) @cfg.loop.body
) @cfg.loop

(loop_statement
  (sequence_of_statements) @cfg.loop.body
) @cfg.loop

; ---------------------------------------------------------------------------
; exception handling (Ada uses exception block in begin/end)
; ---------------------------------------------------------------------------

(handled_sequence_of_statements
  (sequence_of_statements) @cfg.try.body
  (exception_handler) @cfg.try.catch
) @cfg.try

(exception_handler) @cfg.try.catch

; ---------------------------------------------------------------------------
; Exits
; ---------------------------------------------------------------------------

(return_statement) @cfg.exit.return

(exit_statement) @cfg.exit.break

(goto_statement) @cfg.exit.throw

(raise_statement) @cfg.exit.throw
