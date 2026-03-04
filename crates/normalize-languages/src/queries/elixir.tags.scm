; Elixir tags query
; Covers: def/defp/defmacro functions and defmodule modules
;
; In Elixir's tree-sitter grammar, all definitions are represented as `call`
; nodes. The target (def, defp, defmodule, etc.) is an `identifier` child.
; Function names appear as the first argument (a `call` or `identifier` node
; inside `arguments`).

; Public functions: def <name>(...)
(call
  target: (identifier) @_kw
  (arguments
    (call
      target: (identifier) @name))
  (#eq? @_kw "def")) @definition.function

; Private functions: defp <name>(...)
(call
  target: (identifier) @_kw
  (arguments
    (call
      target: (identifier) @name))
  (#eq? @_kw "defp")) @definition.function

; Public macros: defmacro <name>(...)
(call
  target: (identifier) @_kw
  (arguments
    (call
      target: (identifier) @name))
  (#eq? @_kw "defmacro")) @definition.macro

; Private macros: defmacrop <name>(...)
(call
  target: (identifier) @_kw
  (arguments
    (call
      target: (identifier) @name))
  (#eq? @_kw "defmacrop")) @definition.macro

; Modules: defmodule <Alias>
(call
  target: (identifier) @_kw
  (arguments
    (alias) @name)
  (#eq? @_kw "defmodule")) @definition.module

; Protocols: defprotocol <Alias>
(call
  target: (identifier) @_kw
  (arguments
    (alias) @name)
  (#eq? @_kw "defprotocol")) @definition.interface

; Struct: defstruct (inside a module, struct is the module name — skip name capture)
; Implementation: defimpl <Protocol> for <Type>
(call
  target: (identifier) @_kw
  (arguments
    (alias) @name)
  (#eq? @_kw "defimpl")) @reference.implementation
