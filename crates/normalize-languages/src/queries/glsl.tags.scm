; GLSL tags query
;
; GLSL is C-like: functions use function_declarator, structs use struct_specifier.

(function_definition
  declarator: (function_declarator
    declarator: (identifier) @name)) @definition.function

(struct_specifier
  name: (type_identifier) @name
  body: (_)) @definition.class
