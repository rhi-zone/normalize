; Complexity query for MATLAB
; @complexity — nodes that increase cyclomatic complexity
; @nesting — nodes that increase nesting depth

; Complexity nodes
(if_statement) @complexity
(elseif_clause) @complexity
(switch_statement) @complexity
(case_clause) @complexity
(otherwise_clause) @complexity
(while_statement) @complexity
(for_statement) @complexity
(catch_clause) @complexity

; Nesting nodes
(if_statement) @nesting
(switch_statement) @nesting
(while_statement) @nesting
(for_statement) @nesting
(function_definition) @nesting
