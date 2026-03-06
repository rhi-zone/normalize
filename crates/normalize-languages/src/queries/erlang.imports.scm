; Erlang imports query
; @import       — the entire attribute (for line number)
; @import.path  — the module being imported

; -import(module, [fun/arity, ...]).
(import_attribute
  module: (atom) @import.path) @import
