; Go complexity query
; @complexity — nodes that increase cyclomatic complexity
; @nesting — nodes that increase nesting depth

; Complexity nodes
(if_statement) @complexity
(for_statement) @complexity
(expression_switch_statement) @complexity
(type_switch_statement) @complexity
(select_statement) @complexity
(expression_case) @complexity
(type_case) @complexity
(communication_case) @complexity
(binary_expression) @complexity

; Nesting nodes
(if_statement) @nesting
(for_statement) @nesting
(expression_switch_statement) @nesting
(type_switch_statement) @nesting
(select_statement) @nesting
(function_declaration) @nesting
(method_declaration) @nesting
