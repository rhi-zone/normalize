; Julia tags query
; Julia grammar has few named fields. module_definition has "name:" field,
; but most others use positional children. signature node has no named children.

; Module definitions
(module_definition
  name: (identifier) @name) @definition.module

; Function definitions: function foo(...) ... end
; signature has no named children — captured as @name, node_name() extracts the name
(function_definition
  (signature) @name) @definition.function

; Short function definitions: foo(x) = x + 1
(assignment
  . (call_expression
    . (identifier) @name)) @definition.function

; Macro definitions: macro foo(...) ... end
; Same structure as function_definition
(macro_definition
  (signature) @name) @definition.macro

; Struct definitions: struct Foo ... end
; Name is inside type_head
(struct_definition
  (type_head) @name) @definition.class

; Abstract type definitions: abstract type Foo end
(abstract_definition
  (type_head) @name) @definition.interface
