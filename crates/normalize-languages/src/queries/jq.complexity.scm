; jq complexity query
; @complexity — nodes that increase cyclomatic complexity
; @nesting — nodes that increase nesting depth
;
; In jq, if/reduce/try are anonymous keywords, not named nodes.
; We capture funcdef for complexity (each function definition is a branch).

; Complexity nodes
(funcdef) @complexity
(elif) @complexity
(catch) @complexity

; Nesting nodes
(funcdef) @nesting
(elif) @nesting
