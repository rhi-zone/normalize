; Common Lisp CFG query
; Captures control flow nodes for CFG construction.
; See normalize-cfg for the full capture vocabulary.
; Verified against arborium Common Lisp grammar node types.
;
; Common Lisp is a Lisp — all forms are list_lit nodes with a leading symbol.
; We match specific branching forms: if, when, unless, cond, case, ecase,
; do, dolist, dotimes, loop. throw/return-from are exit forms.

; ---------------------------------------------------------------------------
; if (branch)
; ---------------------------------------------------------------------------

(list_lit
  .
  (sym_lit) @_fn
  .
  (_) @cfg.branch.condition
  .
  (_) @cfg.branch.then
  .
  (_)? @cfg.branch.else
  (#eq? @_fn "if")
) @cfg.branch

; when / unless (branch without else)
(list_lit
  .
  (sym_lit) @_fn
  .
  (_) @cfg.branch.condition
  (#match? @_fn "^(when|unless)$")
) @cfg.branch

; ---------------------------------------------------------------------------
; cond / case / ecase (match-like)
; ---------------------------------------------------------------------------

(list_lit
  .
  (sym_lit) @_fn
  (#match? @_fn "^(cond|case|ecase|typecase|etypecase)$")
) @cfg.match

; ---------------------------------------------------------------------------
; do / dolist / dotimes / loop (loop constructs)
; ---------------------------------------------------------------------------

(list_lit
  .
  (sym_lit) @_fn
  .
  (_) @cfg.loop.condition
  (#match? @_fn "^(do|do\\*|dolist|dotimes|loop)$")
) @cfg.loop

; ---------------------------------------------------------------------------
; handler-case / handler-bind / ignore-errors (exception handling)
; ---------------------------------------------------------------------------

(list_lit
  .
  (sym_lit) @_fn
  .
  (_) @cfg.try.body
  (#match? @_fn "^(handler-case|ignore-errors|with-simple-restart)$")
) @cfg.try

; ---------------------------------------------------------------------------
; Exits
; ---------------------------------------------------------------------------

; throw / error / signal
(list_lit
  .
  (sym_lit) @_fn
  (#match? @_fn "^(throw|error|signal|cerror)$")
) @cfg.exit.throw

; return / return-from
(list_lit
  .
  (sym_lit) @_fn
  (#match? @_fn "^(return|return-from)$")
) @cfg.exit.return
