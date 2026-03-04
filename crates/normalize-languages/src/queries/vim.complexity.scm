; Complexity query for VimScript
; @complexity — nodes that increase cyclomatic complexity
; @nesting — nodes that increase nesting depth

; Complexity nodes
(if_statement) @complexity
(elseif_statement) @complexity
(for_loop) @complexity
(while_loop) @complexity
(catch_clause) @complexity

; Nesting nodes
(if_statement) @nesting
(for_loop) @nesting
(while_loop) @nesting
(function_definition) @nesting
