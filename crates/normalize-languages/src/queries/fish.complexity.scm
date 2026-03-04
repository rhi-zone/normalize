; Complexity query for Fish shell
; @complexity — nodes that increase cyclomatic complexity
; @nesting — nodes that increase nesting depth

; Complexity nodes
(if_statement) @complexity
(else_if_clause) @complexity
(while_statement) @complexity
(for_statement) @complexity
(switch_statement) @complexity
(case_clause) @complexity

; Nesting nodes
(if_statement) @nesting
(while_statement) @nesting
(for_statement) @nesting
(switch_statement) @nesting
(function_definition) @nesting
