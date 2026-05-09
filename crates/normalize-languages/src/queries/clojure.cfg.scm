; Clojure CFG query
; Captures control flow nodes for CFG construction.
; See normalize-cfg for the full capture vocabulary.
; Verified against arborium Clojure grammar node types.
;
; Clojure is a Lisp — all forms are list_lit nodes with a leading symbol.
; We match specific branching forms by name: if, when, cond, case, condp,
; loop, doseq, for, dotimes. Throw/return are via throw/return-from.

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

; when (branch without else)
(list_lit
  .
  (sym_lit) @_fn
  .
  (_) @cfg.branch.condition
  (#eq? @_fn "when")
) @cfg.branch

(list_lit
  .
  (sym_lit) @_fn
  .
  (_) @cfg.branch.condition
  (#eq? @_fn "when-not")
) @cfg.branch

; ---------------------------------------------------------------------------
; cond / condp / case (match-like constructs)
; ---------------------------------------------------------------------------

(list_lit
  .
  (sym_lit) @_fn
  (#match? @_fn "^(cond|condp|case|cond->|cond->|case->)$")
) @cfg.match

; ---------------------------------------------------------------------------
; loop / doseq / for / dotimes (loop constructs)
; ---------------------------------------------------------------------------

(list_lit
  .
  (sym_lit) @_fn
  .
  (_) @cfg.loop.condition
  (#match? @_fn "^(loop|doseq|for|dotimes|while|doall|dorun)$")
) @cfg.loop

; ---------------------------------------------------------------------------
; try / catch / finally
; ---------------------------------------------------------------------------

(list_lit
  .
  (sym_lit) @_fn
  (#eq? @_fn "try")
) @cfg.try

(list_lit
  .
  (sym_lit) @_fn
  (#eq? @_fn "catch")
) @cfg.try.catch

(list_lit
  .
  (sym_lit) @_fn
  (#eq? @_fn "finally")
) @cfg.try.finally

; ---------------------------------------------------------------------------
; Exits
; ---------------------------------------------------------------------------

; throw — Clojure form
(list_lit
  .
  (sym_lit) @_fn
  (#eq? @_fn "throw")
) @cfg.exit.throw
