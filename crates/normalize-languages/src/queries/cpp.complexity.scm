; C++ complexity query
; @complexity — nodes that increase cyclomatic complexity
; @nesting — nodes that increase nesting depth

; Complexity nodes
(if_statement) @complexity
(for_statement) @complexity
(for_range_loop) @complexity
(while_statement) @complexity
(do_statement) @complexity
(switch_statement) @complexity
(case_statement) @complexity
(try_statement) @complexity
(catch_clause) @complexity
(throw_statement) @complexity
"&&" @complexity
"||" @complexity
(conditional_expression) @complexity

; Nesting nodes
(if_statement) @nesting
(for_statement) @nesting
(for_range_loop) @nesting
(while_statement) @nesting
(do_statement) @nesting
(switch_statement) @nesting
(try_statement) @nesting
(function_definition) @nesting
(class_specifier) @nesting
(struct_specifier) @nesting
(namespace_definition) @nesting
(lambda_expression) @nesting
