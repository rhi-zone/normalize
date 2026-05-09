; Lean 4 CFG query
; Captures control flow nodes for CFG construction.
; See normalize-cfg for the full capture vocabulary.
; Verified against arborium Lean 4 grammar node types.
;
; Lean 4 is a dependently-typed proof assistant / functional language.
; Control flow: if-then-else, match, do notation.

; ---------------------------------------------------------------------------
; if / else (branch expression — if_then_else)
; ---------------------------------------------------------------------------

(if_then_else
  condition: (_) @cfg.branch.condition
  thenBody: (_) @cfg.branch.then
  elseBody: (_) @cfg.branch.else
) @cfg.branch

(if_then_else
  condition: (_) @cfg.branch.condition
  thenBody: (_) @cfg.branch.then
  .
) @cfg.branch

; ---------------------------------------------------------------------------
; match (pattern matching)
; ---------------------------------------------------------------------------

(match
  discr: (_) @cfg.match.scrutinee
  (match_alt) @cfg.match.arm
) @cfg.match

; ---------------------------------------------------------------------------
; for / while (do notation loops — Lean 4)
; ---------------------------------------------------------------------------

(do_for
  (term) @cfg.loop.condition
  body: (_) @cfg.loop.body
) @cfg.loop

(do_while
  condition: (_) @cfg.loop.condition
  body: (_) @cfg.loop.body
) @cfg.loop

; ---------------------------------------------------------------------------
; Exits
; ---------------------------------------------------------------------------

(return) @cfg.exit.return

(break) @cfg.exit.break

(continue) @cfg.exit.continue

(throw) @cfg.exit.throw
