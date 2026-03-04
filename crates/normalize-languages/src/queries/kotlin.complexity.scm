; Kotlin complexity query
; @complexity — nodes that increase cyclomatic complexity
; @nesting — nodes that increase nesting depth

; Complexity nodes
(if_expression) @complexity
(for_statement) @complexity
(while_statement) @complexity
(do_while_statement) @complexity
(when_entry) @complexity
(catch_block) @complexity
(elvis_expression) @complexity
(conjunction_expression) @complexity
(disjunction_expression) @complexity

; Nesting nodes
(if_expression) @nesting
(for_statement) @nesting
(while_statement) @nesting
(do_while_statement) @nesting
(when_expression) @nesting
(try_expression) @nesting
(function_declaration) @nesting
(class_declaration) @nesting
