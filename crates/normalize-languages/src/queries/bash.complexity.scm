; Bash complexity query
; @complexity — nodes that increase cyclomatic complexity
; @nesting — nodes that increase nesting depth

; Complexity nodes
(if_statement) @complexity
(elif_clause) @complexity
(for_statement) @complexity
(while_statement) @complexity
(case_statement) @complexity
(case_item) @complexity
(pipeline) @complexity
(list) @complexity

; Nesting nodes
(if_statement) @nesting
(for_statement) @nesting
(while_statement) @nesting
(case_statement) @nesting
(function_definition) @nesting
(subshell) @nesting
