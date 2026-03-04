; Scala complexity query
; @complexity — nodes that increase cyclomatic complexity
; @nesting — nodes that increase nesting depth

; Complexity nodes
(if_expression) @complexity
(match_expression) @complexity
(case_clause) @complexity
(for_expression) @complexity
(while_expression) @complexity
(do_while_expression) @complexity
(try_expression) @complexity
(catch_clause) @complexity
(infix_expression) @complexity

; Nesting nodes
(if_expression) @nesting
(match_expression) @nesting
(for_expression) @nesting
(while_expression) @nesting
(do_while_expression) @nesting
(try_expression) @nesting
(function_definition) @nesting
(class_definition) @nesting
(object_definition) @nesting
(trait_definition) @nesting
(block) @nesting
