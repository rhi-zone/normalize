; Complexity query for ReScript (ML-like JS language)
; @complexity — nodes that increase cyclomatic complexity
; @nesting — nodes that increase nesting depth
;
; ReScript complexity comes from if expressions, switch expressions,
; and individual match arms.

; Complexity nodes
(if_expression) @complexity
(switch_expression) @complexity
(switch_match) @complexity
(ternary_expression) @complexity

; Nesting nodes
(if_expression) @nesting
(switch_expression) @nesting
(function) @nesting
(block) @nesting
