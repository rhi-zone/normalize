; Ruby imports query
; @import       — the entire call statement (for line number)
; @import.path  — the module path string

; require 'module'
(call
  method: (identifier) @_method
  (#eq? @_method "require")
  arguments: (argument_list
    (string
      (string_content) @import.path))) @import

; require_relative 'path'
(call
  method: (identifier) @_method
  (#eq? @_method "require_relative")
  arguments: (argument_list
    (string
      (string_content) @import.path))) @import

; include Module
(call
  method: (identifier) @_method
  (#eq? @_method "include")
  arguments: (argument_list
    (constant) @import.path)) @import

; extend Module
(call
  method: (identifier) @_method
  (#eq? @_method "extend")
  arguments: (argument_list
    (constant) @import.path)) @import

; prepend Module
(call
  method: (identifier) @_method
  (#eq? @_method "prepend")
  arguments: (argument_list
    (constant) @import.path)) @import
