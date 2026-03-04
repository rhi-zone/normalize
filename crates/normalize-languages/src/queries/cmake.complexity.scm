; CMake complexity query
; @complexity — nodes that increase cyclomatic complexity
; @nesting — nodes that increase nesting depth

; Complexity nodes
(if_condition) @complexity
(elseif_command) @complexity
(foreach_loop) @complexity
(while_loop) @complexity

; Nesting nodes
(if_condition) @nesting
(foreach_loop) @nesting
(while_loop) @nesting
