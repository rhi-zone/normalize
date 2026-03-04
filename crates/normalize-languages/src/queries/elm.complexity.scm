; Complexity query for Elm
; @complexity — nodes that increase cyclomatic complexity
; @nesting — nodes that increase nesting depth
;
; Elm is a purely functional language; complexity comes from if-else expressions
; and case-of branches.

; Complexity nodes
(if_else_expr) @complexity
(case_of_branch) @complexity

; Nesting nodes
(if_else_expr) @nesting
(case_of_expr) @nesting
(value_declaration) @nesting
(anonymous_function_expr) @nesting
