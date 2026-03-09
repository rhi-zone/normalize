; Ada complexity query
; @complexity — nodes that increase cyclomatic complexity
; @nesting — nodes that increase nesting depth

; Complexity nodes
(if_statement) @complexity
(if_expression) @complexity
(case_statement) @complexity
(case_expression) @complexity
(elsif_statement_item) @complexity
(case_statement_alternative) @complexity
(case_expression_alternative) @complexity
(loop_statement) @complexity

; Nesting nodes
(if_statement) @nesting
(if_expression) @nesting
(case_statement) @nesting
(case_expression) @nesting
(loop_statement) @nesting
(block_statement) @nesting
