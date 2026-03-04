; Meson imports query
; @import       — the entire subproject()/dependency() call (for line number)
; @import.path  — the first argument (project/package name)

; subproject('foo')
; dependency('glib-2.0')
(normal_command
  (identifier) @_cmd
  (#match? @_cmd "^(subproject|dependency)$")
  (argument) @import.path) @import
