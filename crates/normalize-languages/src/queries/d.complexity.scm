; Complexity query for D
; @complexity — nodes that increase cyclomatic complexity
; @nesting — nodes that increase nesting depth

; Complexity nodes
(if_statement) @complexity
(switch_statement) @complexity
(while_statement) @complexity
(for_statement) @complexity
(foreach_statement) @complexity
(catch) @complexity

; Nesting nodes
(if_statement) @nesting
(switch_statement) @nesting
(while_statement) @nesting
(for_statement) @nesting
(foreach_statement) @nesting
(try_statement) @nesting
(function_literal) @nesting
(class_declaration) @nesting
(struct_declaration) @nesting
(module_declaration) @nesting
