; Rust complexity query
; @complexity — nodes that increase cyclomatic complexity
; @nesting — nodes that increase nesting depth

; Complexity nodes
(if_expression) @complexity
(match_expression) @complexity
(for_expression) @complexity
(while_expression) @complexity
(loop_expression) @complexity
(match_arm) @complexity
(binary_expression) @complexity

; Nesting nodes
(if_expression) @nesting
(match_expression) @nesting
(for_expression) @nesting
(while_expression) @nesting
(loop_expression) @nesting
(function_item) @nesting
(impl_item) @nesting
(trait_item) @nesting
(mod_item) @nesting
