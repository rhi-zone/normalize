; Rust CFG query
; Captures control flow nodes for CFG construction.
; See normalize-cfg for the full capture vocabulary.

; ---------------------------------------------------------------------------
; if / if let (branch)
; ---------------------------------------------------------------------------

(if_expression
  condition: (_) @cfg.branch.condition
  consequence: (_) @cfg.branch.then
  alternative: (_) @cfg.branch.else
) @cfg.branch

(if_expression
  condition: (_) @cfg.branch.condition
  consequence: (_) @cfg.branch.then
  .
  ; no alternative — leave @cfg.branch.else unset
) @cfg.branch

; ---------------------------------------------------------------------------
; match (match)
; ---------------------------------------------------------------------------

(match_expression
  value: (_) @cfg.match.scrutinee
  body: (match_block
    (match_arm) @cfg.match.arm
  )
) @cfg.match

; ---------------------------------------------------------------------------
; while / while let (loop with condition)
; ---------------------------------------------------------------------------

(while_expression
  condition: (_) @cfg.loop.condition
  body: (_) @cfg.loop.body
) @cfg.loop

; ---------------------------------------------------------------------------
; for (loop with condition = synthetic)
; ---------------------------------------------------------------------------

(for_expression
  value: (_) @cfg.loop.condition
  body: (_) @cfg.loop.body
) @cfg.loop

; ---------------------------------------------------------------------------
; loop (unconditional — no @cfg.loop.condition)
; ---------------------------------------------------------------------------

(loop_expression
  body: (_) @cfg.loop.body
) @cfg.loop

; ---------------------------------------------------------------------------
; Exits
; ---------------------------------------------------------------------------

(return_expression) @cfg.exit.return

(break_expression) @cfg.exit.break

(continue_expression) @cfg.exit.continue

; panic!, todo!, unreachable! — treat as throw
(macro_invocation
  macro: (identifier) @_macro_name
  (#match? @_macro_name "^(panic|todo|unreachable|unimplemented)$")
) @cfg.exit.throw

; ---------------------------------------------------------------------------
; Def/use sites
; ---------------------------------------------------------------------------

; let x = ... — immutable local variable definition
(let_declaration
  pattern: (identifier) @cfg.def.name
) @cfg.def

; let mut x = ... — mutable local variable definition
(let_declaration
  (mutable_specifier)
  (identifier) @cfg.def.name
) @cfg.def

; x = ... — assignment (re-definition)
(assignment_expression
  left: (identifier) @cfg.def.name
) @cfg.def
