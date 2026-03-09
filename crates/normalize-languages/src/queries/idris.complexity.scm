; Complexity query for Idris (dependently-typed functional language)
; @complexity — nodes that increase cyclomatic complexity
; @nesting — nodes that increase nesting depth
;
; Idris complexity comes from if-then-else expressions, case expressions,
; and individual case alternatives.

; Complexity nodes
(exp_if) @complexity
(exp_case) @complexity
(alt) @complexity

; Nesting nodes
(exp_if) @nesting
(exp_case) @nesting
(function) @nesting
(exp_lambda) @nesting
