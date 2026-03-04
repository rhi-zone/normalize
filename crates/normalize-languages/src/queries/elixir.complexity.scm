; Elixir complexity query
; @complexity — nodes that increase cyclomatic complexity
; @nesting — nodes that increase nesting depth
;
; In Elixir's tree-sitter grammar, control flow constructs (if, case, cond,
; with, for, unless) are represented as call nodes rather than dedicated AST
; nodes — they are macros, not special forms. Binary operators cover and/or/&&/||.

; Complexity nodes — calls and operators that branch execution
(call) @complexity
(binary_operator) @complexity

; Nesting nodes — blocks and anonymous functions that introduce new scopes
(call) @nesting
(do_block) @nesting
(anonymous_function) @nesting
