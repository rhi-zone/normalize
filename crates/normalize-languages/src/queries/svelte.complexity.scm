; Svelte complexity query
; @complexity — nodes that increase cyclomatic complexity
; @nesting — nodes that increase nesting depth

; Complexity nodes
(if_statement) @complexity
(each_statement) @complexity
(else_if_block) @complexity

; Nesting nodes
(if_statement) @nesting
(each_statement) @nesting
