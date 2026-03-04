; Elixir calls query
; @call — call expression nodes
; @call.qualifier — qualifier/receiver for remote calls
;
; In Elixir's tree-sitter grammar, all function calls are represented as
; `call` nodes. The target can be an identifier (local call) or a dot node
; (remote call: Module.func() or var.func()). This includes macro calls
; like def/defmodule/if/case — they are all calls in Elixir.

; Local call: foo() or foo(args)
(call
  target: (identifier) @call)

; Remote call: Module.func() or obj.func()
(call
  target: (dot
    left: (_) @call.qualifier
    right: (identifier) @call))
