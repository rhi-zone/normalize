; Source: arborium (tree-sitter-capnp). Convention converted from @scope/@definition/@reference
; to @local.scope/@local.definition/@local.reference.
; Cap'n Proto is a schema IDL; "scope" here means a struct/interface body.

; Scopes
[
  (message)
  (struct)
  (interface)
  (enum)
  (method_parameters)
  (named_return_types)
  (group)
  (union)
] @local.scope

; Definitions
(annotation_definition_identifier) @local.definition
(const_identifier) @local.definition
(enum (enum_identifier) @local.definition)
(enum_member) @local.definition
(field_identifier) @local.definition
(method_identifier) @local.definition
(namespace) @local.definition
(param_identifier) @local.definition
(return_identifier) @local.definition
(group (type_identifier) @local.definition)
(struct (type_identifier) @local.definition)
(union (type_identifier) @local.definition)
(interface (type_identifier) @local.definition)

; Generics
(struct
  (generics
    (generic_parameters
      (generic_identifier) @local.definition)))

(interface
  (generics
    (generic_parameters
      (generic_identifier) @local.definition)))

(method
  (implicit_generics
    (implicit_generic_parameters
      (generic_identifier) @local.definition)))

(method
  (generics
    (generic_parameters
      (generic_identifier) @local.definition)))

; References
(extend_type) @local.reference
(field_type) @local.reference
(custom_type (type_identifier) @local.reference)
