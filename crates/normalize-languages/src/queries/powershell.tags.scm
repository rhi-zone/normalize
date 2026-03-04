; PowerShell tags query

(function_statement
  name: (command_name) @name) @definition.function

(class_statement
  name: (type_identifier) @name) @definition.class

(enum_statement
  name: (type_identifier) @name) @definition.type
