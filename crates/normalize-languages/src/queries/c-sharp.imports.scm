; C# imports query
; @import       — the entire using directive (for line number)
; @import.path  — the namespace or type path
; @import.alias — alias name (for `using Alias = Namespace;`)

; using Namespace;
(using_directive
  (identifier) @import.path) @import

; using Fully.Qualified.Namespace;
(using_directive
  (qualified_name) @import.path) @import

; using Alias = Namespace;
(using_directive
  name: (identifier) @import.alias
  (identifier) @import.path) @import

; using Alias = Fully.Qualified.Namespace;
(using_directive
  name: (identifier) @import.alias
  (qualified_name) @import.path) @import

; using static Fully.Qualified.Type;
(using_directive
  "static"
  (qualified_name) @import.path) @import

; using static Namespace;
(using_directive
  "static"
  (identifier) @import.path) @import
