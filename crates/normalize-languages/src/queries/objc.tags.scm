; Objective-C tags query

(class_interface
  name: (identifier) @name) @definition.class

(class_implementation
  name: (identifier) @name) @definition.class

(protocol_declaration
  name: (identifier) @name) @definition.interface

(method_declaration
  (method_selector
    (keyword_declarator
      keyword: (keyword_argument_declarator) @name))) @definition.method

(function_definition
  declarator: (function_declarator
    declarator: (identifier) @name)) @definition.function

(type_definition
  declarator: (type_identifier) @name) @definition.type

(struct_specifier
  name: (type_identifier) @name
  body: (_)) @definition.class

(enum_specifier
  name: (type_identifier) @name) @definition.type
