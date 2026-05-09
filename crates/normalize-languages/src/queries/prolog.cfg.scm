; Prolog CFG query
; Captures control flow nodes for CFG construction.
; See normalize-cfg for the full capture vocabulary.
; Verified against arborium Prolog grammar node types.
;
; Prolog's control flow is via clause matching, cut (!), and meta-predicates
; (if_then: ->  if_then_else: ;  catch/3). There are no loops/break/continue
; in the imperative sense — recursion replaces iteration.

; ---------------------------------------------------------------------------
; if-then (branch via ->)
; ---------------------------------------------------------------------------

(if_then
  condition: (_) @cfg.branch.condition
  consequence: (_) @cfg.branch.then
) @cfg.branch

; if-then-else (;/2 with ->)
(if_then_else
  condition: (_) @cfg.branch.condition
  consequence: (_) @cfg.branch.then
  alternative: (_) @cfg.branch.else
) @cfg.branch

; ---------------------------------------------------------------------------
; catch/3 (exception handling: catch(Goal, Catcher, Recovery))
; ---------------------------------------------------------------------------

(call
  (atom) @_fn
  (_) @cfg.try.body
  (_) @cfg.try.catch
  (_)
  (#eq? @_fn "catch")
) @cfg.try

; ---------------------------------------------------------------------------
; Exits
; ---------------------------------------------------------------------------

; throw/1
(call
  (atom) @_fn
  (_)
  (#eq? @_fn "throw")
) @cfg.exit.throw
