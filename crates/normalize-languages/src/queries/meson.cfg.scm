; Meson CFG query
; Captures control flow nodes for CFG construction.
; See normalize-cfg for the full capture vocabulary.
; Verified against arborium Meson grammar node types.

; ---------------------------------------------------------------------------
; if / elif / else (branch)
; ---------------------------------------------------------------------------

(if_command
  (if_condition) @cfg.branch.condition
  body: (_) @cfg.branch.then
  (elif_command) @cfg.branch.else
) @cfg.branch

(if_command
  (if_condition) @cfg.branch.condition
  body: (_) @cfg.branch.then
  (else_command
    body: (_) @cfg.branch.else)
) @cfg.branch

(if_command
  (if_condition) @cfg.branch.condition
  body: (_) @cfg.branch.then
  .
) @cfg.branch

; ---------------------------------------------------------------------------
; foreach (loop)
; ---------------------------------------------------------------------------

(foreach_command
  (identifier) @cfg.loop.condition
  body: (_) @cfg.loop.body
) @cfg.loop

(foreach_command
  body: (_) @cfg.loop.body
) @cfg.loop

; ---------------------------------------------------------------------------
; Exits
; ---------------------------------------------------------------------------

(break_command) @cfg.exit.break

(continue_command) @cfg.exit.continue
