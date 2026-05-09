; Emacs Lisp CFG query
; Captures control flow nodes for CFG construction.
; See normalize-cfg for the full capture vocabulary.
; Verified against arborium Emacs Lisp grammar node types.
;
; Emacs Lisp is a Lisp — all forms are list nodes with a leading symbol.
; We match specific forms: if, when, unless, cond, case, while,
; cl-case, cl-loop, dotimes, dolist. throw/error are exit forms.

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
; cond / cl-case / pcase (match-like)
; ---------------------------------------------------------------------------

(list
  .
  (symbol) @_fn
  (#match? @_fn "^(cond|case|cl-case|pcase|pcase-exhaustive)$")
) @cfg.match

; ---------------------------------------------------------------------------
; while / dotimes / dolist / cl-loop (loop constructs)
; ---------------------------------------------------------------------------

(list
  .
  (symbol) @_fn
  .
  (_) @cfg.loop.condition
  (#match? @_fn "^(while|until|dotimes|dolist|cl-loop|cl-do|cl-dolist)$")
) @cfg.loop

; ---------------------------------------------------------------------------
; condition-case / ignore-errors (exception handling)
; ---------------------------------------------------------------------------

(list
  .
  (symbol) @_fn
  .
  (_)
  .
  (_) @cfg.try.body
  (#match? @_fn "^(condition-case|condition-case-unless-debug|ignore-errors)$")
) @cfg.try

; ---------------------------------------------------------------------------
; Exits
; ---------------------------------------------------------------------------

; throw / error / signal
(list
  .
  (symbol) @_fn
  (#match? @_fn "^(throw|error|signal|user-error)$")
) @cfg.exit.throw

; return
(list
  .
  (symbol) @_fn
  (#eq? @_fn "cl-return")
) @cfg.exit.return
