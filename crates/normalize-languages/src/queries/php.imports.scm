; PHP imports query
; @import       — the entire use/include/require statement (for line number)
; @import.path  — the namespace path or file path string
; @import.name  — a single name from a grouped use declaration
; @import.alias — alias after 'as'

; use Namespace\Class;
(namespace_use_declaration
  (namespace_use_clause
    name: (qualified_name) @import.path)) @import

; use Namespace\Class as Alias;
(namespace_use_declaration
  (namespace_use_clause
    name: (qualified_name) @import.path
    (namespace_aliasing_clause
      (name) @import.alias))) @import

; use function Namespace\func;
(namespace_function_use_declaration
  (namespace_use_clause
    name: (qualified_name) @import.path)) @import

; use const Namespace\CONST;
(namespace_const_use_declaration
  (namespace_use_clause
    name: (qualified_name) @import.path)) @import

; include 'file.php' or require 'file.php'
(include_expression
  (string
    (string_value) @import.path)) @import

; include_once / require_once
(include_once_expression
  (string
    (string_value) @import.path)) @import
