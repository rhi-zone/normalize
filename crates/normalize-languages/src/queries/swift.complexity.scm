; Swift complexity query
; @complexity — nodes that increase cyclomatic complexity
; @nesting — nodes that increase nesting depth

; Complexity nodes
(if_statement) @complexity
(for_statement) @complexity
(while_statement) @complexity
(repeat_while_statement) @complexity
(switch_statement) @complexity
(catch_block) @complexity
(ternary_expression) @complexity
(nil_coalescing_expression) @complexity

; Nesting nodes
(if_statement) @nesting
(for_statement) @nesting
(while_statement) @nesting
(repeat_while_statement) @nesting
(switch_statement) @nesting
(do_statement) @nesting
(function_declaration) @nesting
(class_declaration) @nesting
(lambda_literal) @nesting
