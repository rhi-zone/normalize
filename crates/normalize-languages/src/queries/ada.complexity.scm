; Ada complexity query
; @complexity — nodes that increase cyclomatic complexity
; @nesting — nodes that increase nesting depth

; Complexity nodes
(case_expression) @complexity
(if_expression) @complexity
(case_expression_alternative) @complexity

; Nesting nodes
(if_expression) @nesting
(case_expression) @nesting
(loop_statement) @nesting
(block_statement) @nesting
