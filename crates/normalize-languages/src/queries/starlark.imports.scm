; Starlark (Bazel/Buck) imports query
; @import       — the entire load statement (for line number)
; @import.path  — the .bzl file path
; @import.name  — a single symbol being loaded

; load("//path/to:file.bzl", "symbol", "other")
; First string child is the path; subsequent string children are names.
(load_statement
  (string) @import.path) @import

; load("//path/to:file.bzl", local = "symbol")
(load_statement
  (aliased_load
    alias: (identifier) @import.name)) @import
