; Complexity query for SQL
; @complexity — nodes that increase cyclomatic complexity
; @nesting — nodes that increase nesting depth
;
; SQL complexity comes from CASE expressions (each WHEN branch), JOINs,
; WHERE and HAVING clauses that add branching conditions.

; Complexity nodes
(when_clause) @complexity
(join) @complexity
(where) @complexity
(having) @complexity

; Nesting nodes
(select) @nesting
(subquery) @nesting
