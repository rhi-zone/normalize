; PHP imports query
; @import       — the entire use/include/require statement (for line number)
; @import.path  — the namespace path or file path string
; @import.name  — a single name from a grouped use declaration
; @import.alias — alias after 'as'

; use Namespace\Class;
(namespace_use_declaration
  (namespace_use_clause
    (qualified_name) @import.path)) @import

; use Namespace\Class as Alias;
(namespace_use_declaration
  (namespace_use_clause
    (qualified_name) @import.path
    alias: (name) @import.alias)) @import

; use function Namespace\func; (also handled by namespace_use_declaration above)

; use const Namespace\CONST; (handled by namespace_use_declaration above)

; include 'file.php' or require 'file.php'
(include_expression
  (string
    (string_content) @import.path)) @import

; include_once / require_once
(include_once_expression
  (string
    (string_content) @import.path)) @import
