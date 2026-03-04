; Complexity query for Haskell
; @complexity — nodes that increase cyclomatic complexity
; @nesting — nodes that increase nesting depth
;
; In Haskell's tree-sitter grammar, conditional, case, match, guard, and lambda
; are represented as expressions rather than statements.

; Complexity nodes
(conditional) @complexity
(case) @complexity
(match) @complexity
(guard) @complexity
(lambda) @complexity

; Nesting nodes
(conditional) @nesting
(case) @nesting
(match) @nesting
(function) @nesting
(class) @nesting
(instance) @nesting
