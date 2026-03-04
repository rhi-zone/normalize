; Julia tags query
; Covers: functions, short functions, macros, structs, abstract types, modules

; Function definitions: function foo(...) ... end
(function_definition
  name: (identifier) @name) @definition.function

; Short function definitions: foo(x) = x + 1
(assignment
  . (call_expression
    . (identifier) @name)) @definition.function

; Macro definitions: macro foo(...) ... end
(macro_definition
  name: (identifier) @name) @definition.macro

; Struct definitions (mutable and immutable)
(struct_definition
  name: (identifier) @name) @definition.class

; Abstract type definitions
(abstract_definition
  name: (identifier) @name) @definition.interface

; Module definitions
(module_definition
  name: (identifier) @name) @definition.module
