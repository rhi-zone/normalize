; Complexity query for SQL
; @complexity — nodes that increase cyclomatic complexity
; @nesting — nodes that increase nesting depth
;
; SQL complexity comes from CASE expressions (each WHEN branch), JOINs,
; WHERE and HAVING clauses that add branching conditions.

; Complexity nodes
(case_expression) @complexity
(when_clause) @complexity
(join_clause) @complexity
(where_clause) @complexity
(having_clause) @complexity

; Nesting nodes
(select_statement) @nesting
(subquery) @nesting
(case_expression) @nesting
