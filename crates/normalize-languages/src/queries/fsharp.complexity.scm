; Complexity query for F#
; @complexity — nodes that increase cyclomatic complexity
; @nesting — nodes that increase nesting depth
;
; F# is a functional-first language; complexity comes from if/elif expressions,
; match rules, loops, try blocks, and boolean infix operators (&&, ||).

; Complexity nodes
(if_expression) @complexity
(rule) @complexity
(for_expression) @complexity
(while_expression) @complexity
(try_expression) @complexity
(infix_expression) @complexity

; Nesting nodes
(if_expression) @nesting
(for_expression) @nesting
(while_expression) @nesting
(try_expression) @nesting
(function_or_value_defn) @nesting
(member_defn) @nesting
(module_defn) @nesting
(type_definition) @nesting
