; Complexity query for PowerShell
; @complexity — nodes that increase cyclomatic complexity
; @nesting — nodes that increase nesting depth

; Complexity nodes
(if_statement) @complexity
(elseif_clause) @complexity
(while_statement) @complexity
(for_statement) @complexity
(foreach_statement) @complexity
(switch_statement) @complexity
(catch_clause) @complexity

; Nesting nodes
(if_statement) @nesting
(while_statement) @nesting
(for_statement) @nesting
(foreach_statement) @nesting
(switch_statement) @nesting
(try_statement) @nesting
(function_statement) @nesting
(class_statement) @nesting
