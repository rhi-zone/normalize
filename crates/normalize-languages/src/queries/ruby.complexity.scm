; Ruby complexity query
; @complexity — nodes that increase cyclomatic complexity
; @nesting — nodes that increase nesting depth

; Complexity nodes
(if) @complexity
(unless) @complexity
(case) @complexity
(when) @complexity
(while) @complexity
(until) @complexity
(for) @complexity
(begin) @complexity
(rescue) @complexity
"and" @complexity
"or" @complexity
(conditional) @complexity

; Nesting nodes
(if) @nesting
(unless) @nesting
(case) @nesting
(while) @nesting
(until) @nesting
(for) @nesting
(begin) @nesting
(method) @nesting
(singleton_method) @nesting
(class) @nesting
(module) @nesting
(do_block) @nesting
(block) @nesting
