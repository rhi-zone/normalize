; Complexity query for Julia
; @complexity — nodes that increase cyclomatic complexity
; @nesting — nodes that increase nesting depth

; Complexity nodes
(if_statement) @complexity
(elseif_clause) @complexity
(for_statement) @complexity
(while_statement) @complexity
(ternary_expression) @complexity

; Nesting nodes
(if_statement) @nesting
(for_statement) @nesting
(while_statement) @nesting
(try_statement) @nesting
(function_definition) @nesting
(macro_definition) @nesting
(module_definition) @nesting
(struct_definition) @nesting
