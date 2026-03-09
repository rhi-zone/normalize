; Groovy imports query
; @import       — the entire import statement (for line number)
; @import.path  — the package/class being imported (qualified_name field)
; @import.alias — alias after 'as'
; @import.glob  — wildcard marker (import foo.*)
;
; Grammar: groovy_import has field `import: qualified_name` for the path,
; optional `import_alias: identifier` for aliases, and optional
; `wildcard_import` child for `.*` imports.

; import com.example.Foo
(groovy_import
  import: (qualified_name) @import.path) @import

; import com.example.*
(groovy_import
  import: (qualified_name) @import.path
  (wildcard_import)) @import.glob @import

; import com.example.Foo as Bar
(groovy_import
  import: (qualified_name) @import.path
  import_alias: (identifier) @import.alias) @import
