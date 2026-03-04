; Prolog complexity query
; @complexity — nodes that increase cyclomatic complexity
; @nesting — nodes that increase nesting depth

; Complexity nodes (each clause adds complexity via pattern matching)
(clause_term) @complexity

; Nesting nodes
(clause_term) @nesting
