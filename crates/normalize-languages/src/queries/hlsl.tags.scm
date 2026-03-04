; HLSL tags query
;
; HLSL is C-like: functions use function_declarator, structs and cbuffers are type containers.

(function_definition
  declarator: (function_declarator
    declarator: (identifier) @name)) @definition.function

(struct_specifier
  name: (type_identifier) @name
  body: (_)) @definition.class

(cbuffer_specifier
  name: (type_identifier) @name
  body: (_)) @definition.class
