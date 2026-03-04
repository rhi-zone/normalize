; JavaScript complexity query
; @complexity — nodes that increase cyclomatic complexity
; @nesting — nodes that increase nesting depth

; Complexity nodes
(if_statement) @complexity
(for_statement) @complexity
(for_in_statement) @complexity
(while_statement) @complexity
(do_statement) @complexity
(switch_case) @complexity
(catch_clause) @complexity
(ternary_expression) @complexity
(binary_expression) @complexity

; Nesting nodes
(if_statement) @nesting
(for_statement) @nesting
(for_in_statement) @nesting
(while_statement) @nesting
(do_statement) @nesting
(switch_statement) @nesting
(try_statement) @nesting
(function_declaration) @nesting
(method_definition) @nesting
(class_declaration) @nesting
