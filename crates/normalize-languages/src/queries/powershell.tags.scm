; PowerShell tags query

; function_statement has no name: field; function_name is a child node
(function_statement
  (function_name) @name) @definition.function

; class_statement has no name: field; simple_name is a child node
(class_statement
  (simple_name) @name) @definition.class

(enum_statement
  (simple_name) @name) @definition.type
