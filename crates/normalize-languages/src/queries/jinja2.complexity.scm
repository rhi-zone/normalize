; Jinja2 complexity query
; @complexity — nodes that increase cyclomatic complexity
; @nesting     — nodes that increase nesting depth

; Complexity nodes
(for_statement) @complexity
(if_statement) @complexity
(elif_clause) @complexity

; Nesting nodes
(for_statement) @nesting
(if_statement) @nesting
(macro_statement) @nesting
(call_statement) @nesting
