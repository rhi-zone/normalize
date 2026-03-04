; SCSS complexity query
; @complexity — nodes that increase cyclomatic complexity
; @nesting — nodes that increase nesting depth

; Complexity nodes
(if_statement) @complexity
(for_statement) @complexity
(each_statement) @complexity
(while_statement) @complexity

; Nesting nodes
(if_statement) @nesting
(for_statement) @nesting
(each_statement) @nesting
(while_statement) @nesting
