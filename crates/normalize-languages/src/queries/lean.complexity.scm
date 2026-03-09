; Complexity query for Lean 4
; @complexity — nodes that increase cyclomatic complexity
; @nesting — nodes that increase nesting depth
;
; Lean complexity comes from if-then-else expressions, match expressions,
; and individual match alternatives.

; Complexity nodes
(if_then_else) @complexity
(match) @complexity
(match_alt) @complexity

; Nesting nodes
(if_then_else) @nesting
(match) @nesting
(def) @nesting
(theorem) @nesting
(do) @nesting
