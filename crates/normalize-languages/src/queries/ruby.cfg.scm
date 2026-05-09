; Ruby CFG query
; Captures control flow nodes for CFG construction.
; See normalize-cfg for the full capture vocabulary.
; Verified against arborium Ruby grammar node types.
;
; Ruby uses expression-oriented constructs: if/unless are both used.
; next is the continue equivalent, raise is the throw equivalent.

; ---------------------------------------------------------------------------
; if / unless (branch)
; ---------------------------------------------------------------------------

(if
  condition: (_) @cfg.branch.condition
  consequence: (_) @cfg.branch.then
  alternative: (_) @cfg.branch.else
) @cfg.branch

(if
  condition: (_) @cfg.branch.condition
  consequence: (_) @cfg.branch.then
  .
  ; no alternative
) @cfg.branch

; unless is an inverted if
(unless
  condition: (_) @cfg.branch.condition
  consequence: (_) @cfg.branch.then
  alternative: (_) @cfg.branch.else
) @cfg.branch

(unless
  condition: (_) @cfg.branch.condition
  consequence: (_) @cfg.branch.then
  .
) @cfg.branch

; ---------------------------------------------------------------------------
; case / when (match)
; ---------------------------------------------------------------------------

(case
  value: (_) @cfg.match.scrutinee
  (when) @cfg.match.arm
) @cfg.match

; case/in (pattern matching, Ruby 3+)
(case
  value: (_) @cfg.match.scrutinee
  (in_pattern) @cfg.match.arm
) @cfg.match

; ---------------------------------------------------------------------------
; while / until (loop)
; ---------------------------------------------------------------------------

(while
  condition: (_) @cfg.loop.condition
  body: (_) @cfg.loop.body
) @cfg.loop

(until
  condition: (_) @cfg.loop.condition
  body: (_) @cfg.loop.body
) @cfg.loop

; ---------------------------------------------------------------------------
; for (loop over collection)
; ---------------------------------------------------------------------------

(for
  pattern: (_) @cfg.loop.condition
  body: (_) @cfg.loop.body
) @cfg.loop

; ---------------------------------------------------------------------------
; begin / rescue / ensure (exception handling)
; ---------------------------------------------------------------------------

(begin
  body: (_) @cfg.try.body
) @cfg.try

(rescue
  (rescue_modifier) @cfg.try.catch
) @cfg.try.catch

(rescue) @cfg.try.catch

(ensure) @cfg.try.finally

; ---------------------------------------------------------------------------
; Exits
; ---------------------------------------------------------------------------

(return) @cfg.exit.return

(break) @cfg.exit.break

(next) @cfg.exit.continue

(raise) @cfg.exit.throw
