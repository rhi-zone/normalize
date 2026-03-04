; WIT (WebAssembly Interface Types) imports query
; @import       — the entire use item (for line number)
; @import.path  — the interface path being used
; @import.name  — a specific item imported

; use wasi:io/streams.{input-stream, output-stream}
(use_item) @import.path @import
