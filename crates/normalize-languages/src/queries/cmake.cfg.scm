; CMake CFG query
; Captures control flow nodes for CFG construction.
; See normalize-cfg for the full capture vocabulary.
; Verified against arborium CMake grammar node types.

; ---------------------------------------------------------------------------
; if / elseif / else (branch)
; ---------------------------------------------------------------------------

(if_condition
  condition: (_) @cfg.branch.condition
  body: (_) @cfg.branch.then
  (elseif_command) @cfg.branch.else
) @cfg.branch

(if_condition
  condition: (_) @cfg.branch.condition
  body: (_) @cfg.branch.then
  (else_command
    body: (_) @cfg.branch.else)
) @cfg.branch

(if_condition
  condition: (_) @cfg.branch.condition
  body: (_) @cfg.branch.then
  .
) @cfg.branch

; ---------------------------------------------------------------------------
; foreach (loop)
; ---------------------------------------------------------------------------

(foreach_loop
  (argument) @cfg.loop.condition
  body: (_) @cfg.loop.body
) @cfg.loop

(foreach_loop
  body: (_) @cfg.loop.body
) @cfg.loop

; ---------------------------------------------------------------------------
; while (loop with condition)
; ---------------------------------------------------------------------------

(while_loop
  condition: (_) @cfg.loop.condition
  body: (_) @cfg.loop.body
) @cfg.loop

; ---------------------------------------------------------------------------
; Exits
; ---------------------------------------------------------------------------

(break_command) @cfg.exit.break

(continue_command) @cfg.exit.continue

(return_command) @cfg.exit.return
