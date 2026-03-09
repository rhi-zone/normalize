; C# complexity query
; @complexity — nodes that increase cyclomatic complexity
; @nesting — nodes that increase nesting depth

; Complexity nodes
(if_statement) @complexity
(for_statement) @complexity
(foreach_statement) @complexity
(while_statement) @complexity
(do_statement) @complexity
(switch_section) @complexity
(catch_clause) @complexity
(conditional_expression) @complexity
(binary_expression) @complexity

; Nesting nodes
(if_statement) @nesting
(for_statement) @nesting
(foreach_statement) @nesting
(while_statement) @nesting
(do_statement) @nesting
(switch_statement) @nesting
(try_statement) @nesting
(method_declaration) @nesting
(class_declaration) @nesting
(lambda_expression) @nesting
