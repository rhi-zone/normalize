; Complexity query for Groovy
; @complexity — nodes that increase cyclomatic complexity
; @nesting — nodes that increase nesting depth

; Complexity nodes
(if_statement) @complexity
(for_loop) @complexity
(for_in_loop) @complexity
(while_loop) @complexity
(switch_statement) @complexity
(case) @complexity
(ternary_op) @complexity

; Nesting nodes
(if_statement) @nesting
(for_loop) @nesting
(for_in_loop) @nesting
(while_loop) @nesting
(switch_statement) @nesting
(function_definition) @nesting
(closure) @nesting
(class_definition) @nesting
