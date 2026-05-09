; Erlang CFG query
; Captures control flow nodes for CFG construction.
; See normalize-cfg for the full capture vocabulary.
; Verified against arborium Erlang grammar node types.
;
; Erlang uses case_expr/if_expr/receive_expr/try_expr for control flow.
; Pattern matching in function clauses is the primary branching mechanism.
; No imperative break/continue — tail recursion replaces loops.

; ---------------------------------------------------------------------------
; if (branch — guards as conditions)
; ---------------------------------------------------------------------------

(if_expr
  (if_clause) @cfg.branch.then
) @cfg.branch

; ---------------------------------------------------------------------------
; case (match)
; ---------------------------------------------------------------------------

(case_expr
  expr: (_) @cfg.match.scrutinee
  (cr_clause) @cfg.match.arm
) @cfg.match

; ---------------------------------------------------------------------------
; receive (message-passing — treated as match with optional after)
; ---------------------------------------------------------------------------

(receive_expr
  (cr_clause) @cfg.match.arm
) @cfg.match

; ---------------------------------------------------------------------------
; try / catch / after
; ---------------------------------------------------------------------------

(try_expr
  exprs: (_) @cfg.try.body
) @cfg.try

(catch_clause) @cfg.try.catch

(after_clause) @cfg.try.finally

; ---------------------------------------------------------------------------
; Exits
; ---------------------------------------------------------------------------

; throw/exit/error are built-in calls in Erlang
(call
  (remote
    module: (atom) @_m
    function: (atom) @_fn)
  (#eq? @_m "erlang")
  (#match? @_fn "^(throw|exit|error)$")
) @cfg.exit.throw
