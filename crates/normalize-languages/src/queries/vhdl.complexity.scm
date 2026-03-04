; VHDL complexity query
; @complexity — nodes that increase cyclomatic complexity
; @nesting — nodes that increase nesting depth

; Complexity nodes
(if_statement) @complexity
(case_statement) @complexity
(loop_statement) @complexity

; Nesting nodes
(if_statement) @nesting
(case_statement) @nesting
(loop_statement) @nesting
(function_body) @nesting
(procedure_body) @nesting
