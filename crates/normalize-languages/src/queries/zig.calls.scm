; Zig calls query
; @call — call expression nodes
; @call.qualifier — qualifier/receiver for method calls
;
; Zig uses PascalCase node names. Regular calls are SuffixExpr nodes with
; a FnCallArguments child. Builtin calls (@import, @This, etc.) are
; builtin_call_expression nodes.

; Regular call: func() or obj.method()
; SuffixExpr wraps both the callee and the argument list
(SuffixExpr
  (IDENTIFIER) @call
  (FnCallArguments))

; Field (method) call: obj.method()
(SuffixExpr
  (_) @call.qualifier
  (SuffixOp)
  (IDENTIFIER) @call
  (FnCallArguments))

; Builtin call: @import("file"), @This(), etc.
; Builtins use BUILTINIDENTIFIER inside a SuffixExpr
(SuffixExpr
  (BUILTINIDENTIFIER) @call
  (FnCallArguments))
