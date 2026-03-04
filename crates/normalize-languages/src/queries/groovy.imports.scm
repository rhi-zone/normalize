; Groovy imports query
; @import       — the entire import statement (for line number)
; @import.path  — the package/class being imported
; @import.glob  — wildcard marker (import foo.*)

; import com.example.Foo
(groovy_import
  (dotted_identifier) @import.path) @import

; import com.example.*
(groovy_import
  (wildcard_import
    (dotted_identifier) @import.path) @import.glob) @import
