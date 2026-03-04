; Agda complexity query
; @complexity — nodes that increase cyclomatic complexity
; @nesting — nodes that increase nesting depth

; Complexity nodes (pattern matching adds complexity)
(function) @complexity
(lambda_clause) @complexity

; Nesting nodes
(function) @nesting
(lambda_clause) @nesting
