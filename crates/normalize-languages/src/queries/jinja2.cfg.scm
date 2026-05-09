; Jinja2 CFG query
; Captures control flow nodes for CFG construction.
; See normalize-cfg for the full capture vocabulary.
; Verified against installed Jinja2 grammar by running:
;   normalize syntax ast /tmp/sample.jinja2
;
; Node types confirmed: if_statement, elif_clause, else_clause, for_statement,
; endfor, endif.

; ---------------------------------------------------------------------------
; if / elif / else (branch)
; ---------------------------------------------------------------------------

(if_statement
  (_) @cfg.branch.condition
  (elif_clause) @cfg.branch.else
) @cfg.branch

(if_statement
  (_) @cfg.branch.condition
  (else_clause) @cfg.branch.else
) @cfg.branch

(if_statement
  (_) @cfg.branch.condition
  .
) @cfg.branch

; ---------------------------------------------------------------------------
; for (loop over collection)
; ---------------------------------------------------------------------------

(for_statement
  (identifier) @cfg.loop.condition
  (identifier) @cfg.loop.condition
) @cfg.loop

(for_statement) @cfg.loop
