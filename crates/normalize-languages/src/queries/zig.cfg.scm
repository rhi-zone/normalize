; Zig CFG query
; Captures control flow nodes for CFG construction.
; See normalize-cfg for the full capture vocabulary.
; Verified against arborium Zig grammar node types.
;
; Zig uses PascalCase node names: IfStatement, ForStatement, WhileStatement,
; SwitchExpr, ReturnStatement, BreakStatement, ContinueStatement.

; ---------------------------------------------------------------------------
; if / else (branch)
; ---------------------------------------------------------------------------

(IfStatement
  condition: (_) @cfg.branch.condition
  body: (_) @cfg.branch.then
  (ElseSuffix
    body: (_) @cfg.branch.else)
) @cfg.branch

(IfStatement
  condition: (_) @cfg.branch.condition
  body: (_) @cfg.branch.then
  .
) @cfg.branch

; ---------------------------------------------------------------------------
; switch (match)
; ---------------------------------------------------------------------------

(SwitchExpr
  condition: (_) @cfg.match.scrutinee
  (SwitchProng) @cfg.match.arm
) @cfg.match

; ---------------------------------------------------------------------------
; for (loop)
; ---------------------------------------------------------------------------

(ForStatement
  (ForPrefix) @cfg.loop.condition
  body: (_) @cfg.loop.body
) @cfg.loop

; ---------------------------------------------------------------------------
; while (loop with condition)
; ---------------------------------------------------------------------------

(WhileStatement
  condition: (_) @cfg.loop.condition
  body: (_) @cfg.loop.body
) @cfg.loop

; ---------------------------------------------------------------------------
; try / catch (error union handling in Zig uses catch, not try blocks)
; ---------------------------------------------------------------------------

; Zig error handling: expr catch |err| { ... }
(UnwrapErrorSuffix
  rhs: (_) @cfg.try.catch
) @cfg.try

; ---------------------------------------------------------------------------
; Exits
; ---------------------------------------------------------------------------

(ReturnStatement) @cfg.exit.return

(BreakStatement) @cfg.exit.break

(ContinueStatement) @cfg.exit.continue
