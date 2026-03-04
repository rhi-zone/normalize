; Complexity query for Starlark (Bazel build language)
; @complexity — nodes that increase cyclomatic complexity
; @nesting — nodes that increase nesting depth
;
; Starlark is Python-like; complexity comes from if statements, for loops,
; and inline conditional (ternary) expressions.

; Complexity nodes
(if_statement) @complexity
(elif_clause) @complexity
(for_statement) @complexity
(conditional_expression) @complexity
"and" @complexity
"or" @complexity

; Nesting nodes
(if_statement) @nesting
(for_statement) @nesting
(function_definition) @nesting
