; jq complexity query
; @complexity — nodes that increase cyclomatic complexity
; @nesting — nodes that increase nesting depth

; Complexity nodes
(if) @complexity
(try) @complexity
(reduce) @complexity

; Nesting nodes
(if) @nesting
(try) @nesting
(reduce) @nesting
