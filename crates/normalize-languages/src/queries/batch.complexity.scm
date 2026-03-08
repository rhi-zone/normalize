; Batch (Windows CMD) complexity query
; @complexity — nodes that increase cyclomatic complexity
; @nesting — nodes that increase nesting depth
;
; The tree-sitter-batch grammar is minimal: it does not model if_statement,
; for_statement, while_statement, or goto as distinct AST nodes. All control
; flow keywords (IF, FOR, GOTO, etc.) are collapsed into the generic `keyword`
; node alongside non-branching commands. Because the grammar cannot distinguish
; branching keywords from other keywords at the node-type level, no @complexity
; or @nesting captures are possible without false positives.
;
; Function definitions (labels starting with :) are the only structural
; containers — captured as @nesting for nesting depth.

; Nesting nodes
(function_definition) @nesting
