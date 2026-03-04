; Starlark (Bazel/Buck) imports query
; @import       — the entire load statement (for line number)
; @import.path  — the .bzl file path
; @import.name  — a single symbol being loaded

; load("//path/to:file.bzl", "symbol", "other")
(load_statement
  (string) @import.path
  (identifier) @import.name) @import

; load("//path/to:file.bzl", local = "symbol")
(load_statement
  (string) @import.path) @import
