; Elm tags query

(value_declaration
  (function_declaration_left
    name: (lower_case_identifier) @name)) @definition.function

(type_alias_declaration
  name: (upper_case_identifier) @name) @definition.type

(type_declaration
  name: (upper_case_identifier) @name) @definition.class
