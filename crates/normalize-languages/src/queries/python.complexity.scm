; Python complexity query
; @complexity — nodes that increase cyclomatic complexity
; @nesting — nodes that increase nesting depth

; Complexity nodes
(if_statement) @complexity
(for_statement) @complexity
(while_statement) @complexity
(try_statement) @complexity
(except_clause) @complexity
(with_statement) @complexity
(match_statement) @complexity
(case_clause) @complexity
"and" @complexity
"or" @complexity
(conditional_expression) @complexity
(list_comprehension) @complexity
(dictionary_comprehension) @complexity
(set_comprehension) @complexity
(generator_expression) @complexity

; Nesting nodes
(if_statement) @nesting
(for_statement) @nesting
(while_statement) @nesting
(try_statement) @nesting
(with_statement) @nesting
(match_statement) @nesting
(function_definition) @nesting
(class_definition) @nesting
