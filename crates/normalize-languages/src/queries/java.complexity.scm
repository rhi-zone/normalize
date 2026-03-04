; Java complexity query
; @complexity — nodes that increase cyclomatic complexity
; @nesting — nodes that increase nesting depth

; Complexity nodes
(if_statement) @complexity
(for_statement) @complexity
(enhanced_for_statement) @complexity
(while_statement) @complexity
(do_statement) @complexity
(switch_label) @complexity
(catch_clause) @complexity
(ternary_expression) @complexity
(binary_expression) @complexity

; Nesting nodes
(if_statement) @nesting
(for_statement) @nesting
(enhanced_for_statement) @nesting
(while_statement) @nesting
(do_statement) @nesting
(switch_expression) @nesting
(try_statement) @nesting
(method_declaration) @nesting
(class_declaration) @nesting
