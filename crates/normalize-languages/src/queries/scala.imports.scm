; Scala imports query
; @import       — the entire import declaration (for line number)
; @import.path  — the stable identifier / package path
; @import.name  — a single imported name from an import selector
; @import.alias — alias for imported name
; @import.glob  — wildcard marker (presence means is_wildcard=true)

; import foo.bar.Baz  (simple import)
(import_declaration
  path: (stable_identifier) @import.path) @import

; import foo.bar.Baz  (identifier only, no qualifier)
(import_declaration
  path: (identifier) @import.path) @import

; import foo.bar.{A, B, C}  (import selectors — one match per name)
(import_declaration
  path: (stable_identifier) @import.path
  (import_selectors
    (import_selector
      name: (identifier) @import.name))) @import

; import foo.bar.{A => B}  (renamed import)
(import_declaration
  path: (stable_identifier) @import.path
  (import_selectors
    (renamed_identifier
      name: (identifier) @import.name
      rename: (identifier) @import.alias))) @import

; import foo.bar._  or  import foo.bar.*  (wildcard)
(import_declaration
  path: (stable_identifier) @import.path
  (import_selectors
    (wildcard) @import.glob)) @import

; import foo._  (wildcard, identifier path)
(import_declaration
  path: (identifier) @import.path
  (import_selectors
    (wildcard) @import.glob)) @import
