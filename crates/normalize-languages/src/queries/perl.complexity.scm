; Complexity query for Perl
; @complexity — nodes that increase cyclomatic complexity
; @nesting — nodes that increase nesting depth

; Complexity nodes
(conditional_statement) @complexity
(loop_statement) @complexity
(for_statement) @complexity
(conditional_expression) @complexity

; Nesting nodes
(conditional_statement) @nesting
(loop_statement) @nesting
(for_statement) @nesting
(subroutine_declaration_statement) @nesting
(package_statement) @nesting
