; Vue complexity query
; @complexity — nodes that increase cyclomatic complexity
; @nesting — nodes that increase nesting depth

; Complexity nodes (directive attributes like v-if/v-for and interpolations)
(directive_attribute) @complexity
(interpolation) @complexity

; Nesting nodes
(directive_attribute) @nesting
(element) @nesting
