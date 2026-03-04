; Batch (Windows CMD) calls query
; @call — call expression nodes
; @call.qualifier — not applicable
;
; Batch has no distinct call expression node type — the grammar only models
; function definitions, variable declarations, and keywords. There is no
; explicit AST node for calling a subroutine (CALL :label or CALL program).
; Returns no matches.
