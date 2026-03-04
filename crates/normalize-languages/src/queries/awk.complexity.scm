; Awk complexity query
; @complexity — nodes that increase cyclomatic complexity
; @nesting — nodes that increase nesting depth

; Complexity nodes
(if_statement) @complexity
(while_statement) @complexity
(for_statement) @complexity
(for_in_statement) @complexity
(ternary_exp) @complexity

; Nesting nodes
(if_statement) @nesting
(while_statement) @nesting
(for_statement) @nesting
(for_in_statement) @nesting
