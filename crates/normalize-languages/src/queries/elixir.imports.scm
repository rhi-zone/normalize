; Elixir imports query
; @import       — the entire call expression (for line number)
; @import.path  — the module alias/atom path
;
; In the tree-sitter-elixir grammar, alias/import/use/require are `call` nodes.
; The keyword is `target: (identifier)`, and the module is an `(alias)` child.

; alias Foo.Bar
(call
  target: (identifier) @_keyword
  (#eq? @_keyword "alias")
  (arguments (alias) @import.path)) @import

; import Foo.Bar
(call
  target: (identifier) @_keyword
  (#eq? @_keyword "import")
  (arguments (alias) @import.path)) @import

; use Foo.Bar
(call
  target: (identifier) @_keyword
  (#eq? @_keyword "use")
  (arguments (alias) @import.path)) @import

; require Foo.Bar
(call
  target: (identifier) @_keyword
  (#eq? @_keyword "require")
  (arguments (alias) @import.path)) @import
