; F# tags query
; Covers: functions/values, members, types, modules

; Function and value definitions
(function_or_value_defn
  (function_declaration_left
    . (identifier) @name)) @definition.function

; Member definitions (methods)
(member_defn
  (method_or_prop_defn
    (identifier) @name)) @definition.method

; Module definitions (named_module wraps the entire file-level module)
(named_module
  (long_identifier
    (identifier) @name)) @definition.module

; Type definitions (records, unions, classes, aliases)
; Use wildcard _ to match record_type_defn, union_type_defn, etc.
(type_definition
  (_
    (type_name
      (identifier) @name))) @definition.class
