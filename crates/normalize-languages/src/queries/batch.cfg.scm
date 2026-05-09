; Batch (Windows CMD) CFG query
; Captures control flow nodes for CFG construction.
; See normalize-cfg for the full capture vocabulary.
; Verified against arborium Batch grammar node types.
;
; The tree-sitter-batch grammar is minimal: it does not model if_statement,
; for_statement, while_statement, or goto as distinct AST nodes. All control
; flow keywords (IF, FOR, GOTO, etc.) are collapsed into the generic `keyword`
; node alongside non-branching commands. No @cfg.branch / @cfg.loop captures
; are possible without false positives.
;
; Only labels (function definitions) are structurally modeled.
; This query intentionally produces no captures for most constructs.
