; Elixir CFG query
; Captures control flow nodes for CFG construction.
; See normalize-cfg for the full capture vocabulary.
; Verified against arborium Elixir grammar node types.
;
; In Elixir's tree-sitter grammar, if/case/cond/with/for/unless are
; represented as call nodes — they are macros, not special forms.
; We match on the specific call names for precision.

; ---------------------------------------------------------------------------
; if / unless (branch)
; ---------------------------------------------------------------------------

(call
  target: (identifier) @_fn
  (do_block
    (stab_clause) @cfg.branch.then
  )
  (#eq? @_fn "if")
) @cfg.branch

(call
  target: (identifier) @_fn
  (do_block
    (stab_clause) @cfg.branch.then
  )
  (#eq? @_fn "unless")
) @cfg.branch

; ---------------------------------------------------------------------------
; case (match)
; ---------------------------------------------------------------------------

(call
  target: (identifier) @_fn
  (arguments ((_) @cfg.match.scrutinee . (_)*))
  (do_block
    (stab_clause) @cfg.match.arm
  )
  (#eq? @_fn "case")
) @cfg.match

; ---------------------------------------------------------------------------
; cond (multi-branch conditional)
; ---------------------------------------------------------------------------

(call
  target: (identifier) @_fn
  (do_block
    (stab_clause) @cfg.branch.then
  )
  (#eq? @_fn "cond")
) @cfg.branch

; ---------------------------------------------------------------------------
; for (comprehension / loop-like construct)
; ---------------------------------------------------------------------------

(call
  target: (identifier) @_fn
  (arguments ((_) @cfg.loop.condition . (_)*))
  (do_block (_) @cfg.loop.body)
  (#eq? @_fn "for")
) @cfg.loop

; ---------------------------------------------------------------------------
; try / rescue / catch / after (exception handling)
; ---------------------------------------------------------------------------

(call
  target: (identifier) @_fn
  (do_block) @cfg.try.body
  (#eq? @_fn "try")
) @cfg.try

(rescue_clause) @cfg.try.catch

(catch_clause) @cfg.try.catch

(after_clause) @cfg.try.finally

; ---------------------------------------------------------------------------
; Exits
; ---------------------------------------------------------------------------

; raise/throw are calls in Elixir
(call
  target: (identifier) @_fn
  (#match? @_fn "^(raise|throw|exit)$")
) @cfg.exit.throw
