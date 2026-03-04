; Erlang calls query
; @call — call expression nodes
; @call.qualifier — qualifier/receiver for remote calls
;
; Erlang has two call forms:
;   - Local call: func(Args)  — represented as `call` with atom target
;   - Remote call: module:func(Args) — represented as `remote` with module and function

; Local call: func(Args)
(call
  target: (atom) @call)

; Remote call: module:func(Args)
(remote
  module: (_) @call.qualifier
  function: (atom) @call)
