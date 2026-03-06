; Elm tags query

(value_declaration
  (function_declaration_left
    (lower_case_identifier) @name)) @definition.function

(type_alias_declaration
  (upper_case_identifier) @name) @definition.type

(type_declaration
  (upper_case_identifier) @name) @definition.class
