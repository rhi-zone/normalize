; TLA+ complexity query
; @complexity — nodes that increase cyclomatic complexity
; @nesting — nodes that increase nesting depth

; Complexity nodes
(if_then_else) @complexity
(case) @complexity
(conj_list) @complexity
(disj_list) @complexity

; Nesting nodes
(if_then_else) @nesting
(case) @nesting
(conj_list) @nesting
(disj_list) @nesting
(operator_definition) @nesting
