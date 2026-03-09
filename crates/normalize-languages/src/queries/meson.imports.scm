; Meson imports query
; @import       — the entire subproject()/dependency() call (for line number)
; @import.path  — the first argument (project/package name)

; subproject('foo')
; dependency('glib-2.0')
; The first positional argument is in a variableunit child containing a string.
(normal_command
  command: (identifier) @_cmd (#match? @_cmd "^(subproject|dependency)$")
  .
  (variableunit
    .
    (string) @import.path)) @import
