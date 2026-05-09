; TLA+ CFG query
; Captures control flow nodes for CFG construction.
; See normalize-cfg for the full capture vocabulary.
; Verified against arborium TLA+ grammar node types.
;
; TLA+ is a formal specification language, not an imperative language.
; Control flow constructs are logical: IF-THEN-ELSE, CASE expressions,
; conjunction/disjunction lists. No loops, break, continue, or throw.

; ---------------------------------------------------------------------------
; IF / THEN / ELSE (branch expression)
; ---------------------------------------------------------------------------

(if_then_else
  predicate: (_) @cfg.branch.condition
  then: (_) @cfg.branch.then
  else: (_) @cfg.branch.else
) @cfg.branch

; ---------------------------------------------------------------------------
; CASE (match expression)
; ---------------------------------------------------------------------------

(case
  (case_arm) @cfg.match.arm
) @cfg.match
