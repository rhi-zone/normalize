; OCaml complexity query
; @complexity — nodes that increase cyclomatic complexity
; @nesting — nodes that increase nesting depth

; Complexity nodes
(if_expression) @complexity
(match_expression) @complexity
(match_case) @complexity

; Nesting nodes
(let_expression) @nesting
(module_definition) @nesting
(match_expression) @nesting
