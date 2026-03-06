; OCaml tags query
; Covers: value/function definitions, type definitions, module definitions

; Value and function definitions (OCaml doesn't syntactically distinguish)
(value_definition
  (let_binding
    pattern: (value_name) @name)) @definition.function

; Type definitions (includes records, variants, aliases)
(type_definition
  (type_binding
    name: (type_constructor) @name)) @definition.type

; Module definitions
(module_definition
  (module_binding
    (module_name) @name)) @definition.module

; Module type definitions (signatures = interfaces)
(module_type_definition
  (module_type_name) @name) @definition.interface
