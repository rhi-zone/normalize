; CMake imports query
; @import       — the entire include()/find_package() command (for line number)
; @import.path  — the module/package name (first argument)

; include(SomeModule)
; find_package(SomePackage)
(normal_command
  (identifier) @_cmd
  (#match? @_cmd "^(include|find_package)$")
  (argument) @import.path) @import
