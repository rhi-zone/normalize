; C# imports query
; @import       — the entire using directive (for line number)
; @import.path  — the namespace or type path
; @import.alias — alias in 'using Alias = Namespace'

; using Namespace;
(using_directive
  (qualified_name) @import.path) @import

; using Namespace; (simple identifier)
(using_directive
  (identifier) @import.path) @import

; using Alias = Namespace;
(using_directive
  (name_equals
    (identifier) @import.alias)
  (qualified_name) @import.path) @import

; using static Namespace.Type;
(using_directive
  "static"
  (qualified_name) @import.path) @import

; using static Namespace.Type; (simple identifier)
(using_directive
  "static"
  (identifier) @import.path) @import
