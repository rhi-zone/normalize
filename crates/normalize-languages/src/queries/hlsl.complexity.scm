; HLSL complexity query
; @complexity — nodes that increase cyclomatic complexity
; @nesting — nodes that increase nesting depth

; Complexity nodes
(function_definition) @complexity
(if_statement) @complexity
(for_statement) @complexity
(while_statement) @complexity
(switch_statement) @complexity
(case_statement) @complexity
(conditional_expression) @complexity

; Nesting nodes
(if_statement) @nesting
(for_statement) @nesting
(while_statement) @nesting
(switch_statement) @nesting
(function_definition) @nesting
