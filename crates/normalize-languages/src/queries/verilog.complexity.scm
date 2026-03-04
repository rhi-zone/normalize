; Verilog complexity query
; @complexity — nodes that increase cyclomatic complexity
; @nesting — nodes that increase nesting depth

; Complexity nodes
(conditional_statement) @complexity
(case_statement) @complexity
(loop_statement) @complexity

; Nesting nodes
(conditional_statement) @nesting
(case_statement) @nesting
(loop_statement) @nesting
(function_declaration) @nesting
