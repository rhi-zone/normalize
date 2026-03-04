; C# tags query

; Class declarations
(class_declaration
  name: (identifier) @name) @definition.class

; Struct declarations
(struct_declaration
  name: (identifier) @name) @definition.class

; Interface declarations
(interface_declaration
  name: (identifier) @name) @definition.interface

; Enum declarations
(enum_declaration
  name: (identifier) @name) @definition.enum

; Record declarations
(record_declaration
  name: (identifier) @name) @definition.class

; Namespace declarations
(namespace_declaration
  name: (_) @name) @definition.module

; File-scoped namespace declarations
(file_scoped_namespace_declaration
  name: (_) @name) @definition.module

; Method declarations
(method_declaration
  name: (identifier) @name) @definition.method

; Constructor declarations
(constructor_declaration
  name: (identifier) @name) @definition.method

; Property declarations
(property_declaration
  name: (identifier) @name) @definition.method

; Local function statements
(local_function_statement
  name: (identifier) @name) @definition.function
