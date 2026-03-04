; Complexity query for Zsh
; @complexity — nodes that increase cyclomatic complexity
; @nesting — nodes that increase nesting depth
;
; Zsh is very similar to bash; complexity comes from if/elif, for, while,
; case, and pipelines.

; Complexity nodes
(if_statement) @complexity
(elif_clause) @complexity
(for_statement) @complexity
(while_statement) @complexity
(case_statement) @complexity
(case_item) @complexity
(pipeline) @complexity

; Nesting nodes
(if_statement) @nesting
(for_statement) @nesting
(while_statement) @nesting
(case_statement) @nesting
(function_definition) @nesting
(subshell) @nesting
