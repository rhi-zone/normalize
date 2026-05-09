; Scheme CFG query
; Captures control flow nodes for CFG construction.
; See normalize-cfg for the full capture vocabulary.
; Verified against arborium Scheme grammar node types.
;
; Scheme is a Lisp — all forms are list nodes with a leading symbol.
; We match specific forms: if, when, unless, cond, case, do, let-loop.
; call/cc provides non-local exits but is not captured here.

; ---------------------------------------------------------------------------
; if (branch)
; ---------------------------------------------------------------------------

(list
  .
  (symbol) @_fn
  .
  (_) @cfg.branch.condition
  .
  (_) @cfg.branch.then
  .
  (_)? @cfg.branch.else
  (#eq? @_fn "if")
) @cfg.branch

; when / unless (branch without else)
(list
  .
  (symbol) @_fn
  .
  (_) @cfg.branch.condition
  (#match? @_fn "^(when|unless)$")
) @cfg.branch

; ---------------------------------------------------------------------------
; cond / case (match-like)
; ---------------------------------------------------------------------------

(list
  .
  (symbol) @_fn
  (#match? @_fn "^(cond|case|case-lambda)$")
) @cfg.match

; ---------------------------------------------------------------------------
; do / for-each / named-let (loop constructs)
; ---------------------------------------------------------------------------

(list
  .
  (symbol) @_fn
  .
  (_) @cfg.loop.condition
  (#match? @_fn "^(do|for-each|string-for-each|vector-for-each)$")
) @cfg.loop

; ---------------------------------------------------------------------------
; guard / with-exception-handler (exception handling)
; ---------------------------------------------------------------------------

(list
  .
  (symbol) @_fn
  .
  (_) @cfg.try.body
  (#match? @_fn "^(guard|with-exception-handler)$")
) @cfg.try

; ---------------------------------------------------------------------------
; Exits
; ---------------------------------------------------------------------------

; raise / error / raise-continuable
(list
  .
  (symbol) @_fn
  (#match? @_fn "^(raise|error|raise-continuable)$")
) @cfg.exit.throw
