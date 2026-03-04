; Complexity query for Nix
; @complexity — nodes that increase cyclomatic complexity
; @nesting — nodes that increase nesting depth
;
; Nix is a purely functional language; complexity comes from if-then-else
; expressions, assert expressions, and with expressions.

; Complexity nodes
(if_expression) @complexity
(assert_expression) @complexity

; Nesting nodes
(if_expression) @nesting
(with_expression) @nesting
(let_expression) @nesting
(function_expression) @nesting
