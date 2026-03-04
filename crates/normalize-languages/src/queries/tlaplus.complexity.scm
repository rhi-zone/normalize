; TLA+ complexity query
; @complexity — nodes that increase cyclomatic complexity
; @nesting — nodes that increase nesting depth

; Complexity nodes
(if_then_else) @complexity
(case) @complexity

; Nesting nodes
(if_then_else) @nesting
(case) @nesting
(operator_definition) @nesting
