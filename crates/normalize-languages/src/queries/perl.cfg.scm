; Perl CFG query
; Captures control flow nodes for CFG construction.
; See normalize-cfg for the full capture vocabulary.
; Verified against arborium Perl grammar node types.
;
; Perl uses conditional_statement for if/unless/given,
; loop_statement for while/until/for/foreach/do-while.

; ---------------------------------------------------------------------------
; if / unless (branch) — conditional_statement
; ---------------------------------------------------------------------------

(conditional_statement
  condition: (_) @cfg.branch.condition
  consequence: (_) @cfg.branch.then
  alternative: (_) @cfg.branch.else
) @cfg.branch

(conditional_statement
  condition: (_) @cfg.branch.condition
  consequence: (_) @cfg.branch.then
  .
) @cfg.branch

; ---------------------------------------------------------------------------
; given / when (smart match — Perl 5.10+)
; ---------------------------------------------------------------------------

(for_statement
  (expression) @cfg.match.scrutinee
) @cfg.match

; ---------------------------------------------------------------------------
; while / until / foreach (loop)
; ---------------------------------------------------------------------------

(loop_statement
  condition: (_) @cfg.loop.condition
  body: (_) @cfg.loop.body
) @cfg.loop

(loop_statement
  body: (_) @cfg.loop.body
) @cfg.loop

(for_statement
  condition: (_) @cfg.loop.condition
  body: (_) @cfg.loop.body
) @cfg.loop

(for_statement
  body: (_) @cfg.loop.body
) @cfg.loop

; ---------------------------------------------------------------------------
; try / catch (eval-based — use Try::Tiny)
; ---------------------------------------------------------------------------

; Perl's eval {} is the try equivalent — captured as a try block
(eval_expression
  (_) @cfg.try.body
) @cfg.try

; ---------------------------------------------------------------------------
; Exits
; ---------------------------------------------------------------------------

(return_expression) @cfg.exit.return

(last_expression) @cfg.exit.break

(next_expression) @cfg.exit.continue

(redo_expression) @cfg.exit.continue

(die_expression) @cfg.exit.throw
