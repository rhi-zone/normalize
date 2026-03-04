; Meson complexity query
; @complexity — nodes that increase cyclomatic complexity
; @nesting — nodes that increase nesting depth

; Complexity nodes
(if_command) @complexity
(foreach_command) @complexity
(if_condition) @complexity

; Nesting nodes
(if_command) @nesting
(foreach_command) @nesting
