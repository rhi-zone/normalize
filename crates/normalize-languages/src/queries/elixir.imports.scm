; Elixir imports query
; @import       — the entire call expression (for line number)
; @import.path  — the module alias/atom path

; alias Foo.Bar
(call
  target: (identifier) @_keyword
  (#eq? @_keyword "alias")
  arguments: (arguments
    (alias) @import.path)) @import

; import Foo.Bar
(call
  target: (identifier) @_keyword
  (#eq? @_keyword "import")
  arguments: (arguments
    (alias) @import.path)) @import

; use Foo.Bar
(call
  target: (identifier) @_keyword
  (#eq? @_keyword "use")
  arguments: (arguments
    (alias) @import.path)) @import

; require Foo.Bar
(call
  target: (identifier) @_keyword
  (#eq? @_keyword "require")
  arguments: (arguments
    (alias) @import.path)) @import
