; F# tags query
; Covers: functions/values, members, types, modules

; Function and value definitions
(function_or_value_defn
  (function_declaration_left
    name: (identifier) @name)) @definition.function

; Member definitions (methods)
(member_defn
  (method_or_prop_defn
    (identifier) @name)) @definition.method

; Module definitions
(module_defn
  name: (identifier) @name) @definition.module

; Type definitions (records, unions, classes, aliases)
(type_definition
  (type_name
    (identifier) @name)) @definition.class
