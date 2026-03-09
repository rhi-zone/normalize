; PowerShell imports query
; @import       — the entire import command (for line number)
; @import.path  — the module being imported

; Import-Module ModuleName
; Import-Module "path/to/module"
(command
  command_name: (command_name) @_cmd (#match? @_cmd "(?i)^import-module$")
  command_elements: (command_elements) @import.path) @import

; . ./script.ps1 (dot-sourcing)
(command
  command_name: (command_name) @_dot (#eq? @_dot ".")
  command_elements: (command_elements) @import.path) @import
