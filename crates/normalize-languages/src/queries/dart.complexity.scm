; Dart complexity query
; @complexity — nodes that increase cyclomatic complexity
; @nesting — nodes that increase nesting depth

; Complexity nodes
(if_statement) @complexity
(for_statement) @complexity
(while_statement) @complexity
(do_statement) @complexity
(switch_statement_case) @complexity
(catch_clause) @complexity
(conditional_expression) @complexity
(logical_and_expression) @complexity
(logical_or_expression) @complexity

; Nesting nodes
(if_statement) @nesting
(for_statement) @nesting
(while_statement) @nesting
(do_statement) @nesting
(switch_statement) @nesting
(try_statement) @nesting
(function_body) @nesting
(class_definition) @nesting
(function_expression) @nesting
