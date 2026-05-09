; C++ CFG query
; Captures control flow nodes for CFG construction.
; See normalize-cfg for the full capture vocabulary.
; Verified against arborium C++ grammar node types.

; ---------------------------------------------------------------------------
; if / else (branch)
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
; switch (match)
; ---------------------------------------------------------------------------

(switch_statement
  condition: (_) @cfg.match.scrutinee
  body: (compound_statement
    (case_statement) @cfg.match.arm
  )
) @cfg.match

; ---------------------------------------------------------------------------
; for / range-for (loop)
; ---------------------------------------------------------------------------

(for_statement
  condition: (_) @cfg.loop.condition
  body: (_) @cfg.loop.body
) @cfg.loop

(for_statement
  body: (_) @cfg.loop.body
) @cfg.loop

(for_range_loop
  right: (_) @cfg.loop.condition
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
; do-while (loop with condition at end)
; ---------------------------------------------------------------------------

(do_statement
  body: (_) @cfg.loop.body
  condition: (_) @cfg.loop.condition
) @cfg.loop

; ---------------------------------------------------------------------------
; try / catch (exception handling)
; ---------------------------------------------------------------------------

(try_statement
  body: (_) @cfg.try.body
) @cfg.try

(catch_clause) @cfg.try.catch

; ---------------------------------------------------------------------------
; Exits
; ---------------------------------------------------------------------------

(return_statement) @cfg.exit.return

(break_statement) @cfg.exit.break

(continue_statement) @cfg.exit.continue

(throw_statement) @cfg.exit.throw

(goto_statement) @cfg.exit.throw

; ---------------------------------------------------------------------------
; Exception type captures (Phase 4: type-refined exception flow)
; ---------------------------------------------------------------------------

; Thrown type: throw std::runtime_error("msg") → captures "std::runtime_error"
(throw_statement
  (call_expression
    function: (qualified_identifier) @cfg.exit.throw.type))

; Thrown type: throw RuntimeError("msg") → captures "RuntimeError"
(throw_statement
  (call_expression
    function: (identifier) @cfg.exit.throw.type))

; Catch type: catch (const std::runtime_error& e) → captures "std::runtime_error"
(catch_clause
  (parameter_list
    (parameter_declaration
      type: (_) @cfg.try.catch.type)))
