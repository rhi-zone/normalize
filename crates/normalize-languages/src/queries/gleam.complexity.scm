; Complexity query for Gleam
; @complexity — nodes that increase cyclomatic complexity
; @nesting — nodes that increase nesting depth
;
; Gleam is a functional language; complexity comes from case expressions
; and their branches, and if expressions.

; Complexity nodes
(case) @complexity
(case_clause) @complexity
(if) @complexity

; Nesting nodes
(case) @nesting
(if) @nesting
(function) @nesting
(anonymous_function) @nesting
