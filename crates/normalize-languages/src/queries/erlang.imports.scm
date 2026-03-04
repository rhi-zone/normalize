; Erlang imports query
; @import       — the entire attribute (for line number)
; @import.path  — the module being imported or included

; -import(module, [fun/arity, ...]).
(attribute
  name: (atom) @_name (#eq? @_name "import")
  value: (arguments
    (atom) @import.path)) @import

; -include("file.hrl").
(attribute
  name: (atom) @_name (#eq? @_name "include")
  value: (arguments
    (string) @import.path)) @import

; -include_lib("app/include/file.hrl").
(attribute
  name: (atom) @_name (#eq? @_name "include_lib")
  value: (arguments
    (string) @import.path)) @import
