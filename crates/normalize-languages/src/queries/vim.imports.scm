; Vim script imports query
; @import       — the entire source/runtime statement (for line number)
; @import.path  — the file being sourced

; source file.vim
(source_statement
  (_) @import.path) @import

; runtime path/to/file.vim
(runtime_statement
  (_) @import.path) @import
