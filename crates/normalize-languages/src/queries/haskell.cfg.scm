; Haskell CFG query
; Captures control flow nodes for CFG construction.
; See normalize-cfg for the full capture vocabulary.
; Verified against arborium Haskell grammar node types.
;
; Haskell is purely functional — no loops or early exits in the imperative
; sense. Control flow comes from conditional expressions (if-then-else),
; case expressions, and guards. There is no return/break/continue/throw
; in the tree-sitter grammar (these are monadic operations, not AST nodes).

; ---------------------------------------------------------------------------
; if / then / else (branch expression)
; ---------------------------------------------------------------------------

(conditional
  condition: (_) @cfg.branch.condition
  consequence: (_) @cfg.branch.then
  alternative: (_) @cfg.branch.else
) @cfg.branch

; ---------------------------------------------------------------------------
; case / match (pattern matching)
; ---------------------------------------------------------------------------

(case
  subjects: (_) @cfg.match.scrutinee
  (match) @cfg.match.arm
) @cfg.match

; ---------------------------------------------------------------------------
; guard (boolean guards on function equations / case arms)
; ---------------------------------------------------------------------------

; Guards are captured as branch arms — no body capture, guards are the condition
(guard) @cfg.branch
