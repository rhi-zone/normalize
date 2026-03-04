; Erlang complexity query
; @complexity — nodes that increase cyclomatic complexity
; @nesting — nodes that increase nesting depth

; Complexity nodes — case/if/receive branches and guards
(cr_clause) @complexity
(if_clause) @complexity
(catch_clause) @complexity
(guard) @complexity

; Nesting nodes — expressions that introduce nested control flow
(case_expr) @nesting
(if_expr) @nesting
(receive_expr) @nesting
(try_expr) @nesting
(function_clause) @nesting
(fun_clause) @nesting
