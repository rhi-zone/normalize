; Lua complexity query
; @complexity — nodes that increase cyclomatic complexity
; @nesting — nodes that increase nesting depth

; Complexity nodes
(if_statement) @complexity
(elseif_statement) @complexity
(for_statement) @complexity
(while_statement) @complexity
(repeat_statement) @complexity
"and" @complexity
"or" @complexity

; Nesting nodes
(if_statement) @nesting
(for_statement) @nesting
(while_statement) @nesting
(repeat_statement) @nesting
(function_declaration) @nesting
(function_definition) @nesting
