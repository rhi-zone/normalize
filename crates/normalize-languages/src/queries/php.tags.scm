; PHP tags query

(function_definition
  name: (name) @name) @definition.function

(method_declaration
  name: (name) @name) @definition.method

(class_declaration
  name: (name) @name) @definition.class

(interface_declaration
  name: (name) @name) @definition.interface

(trait_declaration
  name: (name) @name) @definition.class

(enum_declaration
  name: (name) @name) @definition.class

(namespace_definition
  name: (namespace_name) @name) @definition.module
