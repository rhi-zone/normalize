; Dockerfile imports query
; @import       — the entire FROM instruction (for line number)
; @import.path  — the base image reference
; @import.alias — the AS stage name

; FROM ubuntu:20.04
(from_instruction
  (image_spec) @import.path) @import

; FROM ubuntu:20.04 AS builder
(from_instruction
  (image_spec) @import.path
  (as_instruction)
  (image_alias) @import.alias) @import
