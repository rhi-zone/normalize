; CMake imports query
; @import       — the entire include()/find_package() command (for line number)
; @import.path  — the module/package name (first argument)

; include(SomeModule)
; find_package(SomePackage)
(normal_command
  (identifier) @_cmd
  (argument_list
    (argument) @import.path)
  (#match? @_cmd "^(include|find_package)$")) @import
