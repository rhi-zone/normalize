; Cap'n Proto imports query
; @import       — the entire import statement (for line number)
; @import.path  — the import path string

; using Foo = import "foo.capnp";
(import
  (import_path) @import.path) @import

; import without named path field (fallback: whole node)
(import) @import.path @import
