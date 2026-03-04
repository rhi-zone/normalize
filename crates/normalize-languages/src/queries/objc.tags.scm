; Objective-C tags query
; class_interface, class_implementation, and protocol_declaration have no name field —
; the name is a positional identifier child.

(class_interface
  (identifier) @name) @definition.class

(class_implementation
  (identifier) @name) @definition.class

(protocol_declaration
  (identifier) @name) @definition.interface

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
