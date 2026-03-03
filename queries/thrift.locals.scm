; Source: arborium (tree-sitter-thrift). Convention converted from @scope/@definition/@reference
; to @local.scope/@local.definition/@local.reference.

; Scopes
[
  (document)
  (definition)
] @local.scope

; References
(identifier) @local.reference

; Definitions
(annotation_identifier) @local.definition
(const_definition (identifier) @local.definition)
(enum_definition "enum" . (identifier) @local.definition
  "{" (identifier) @local.definition "}")
(senum_definition "senum" . (identifier) @local.definition)
(field (identifier) @local.definition)
(function_definition (identifier) @local.definition)
(namespace_declaration "namespace" (namespace_scope) . (_) @local.definition (namespace_uri)?)
(parameter (identifier) @local.definition)
(struct_definition "struct" . (identifier) @local.definition)
(union_definition "union" . (identifier) @local.definition)
(exception_definition "exception" . (identifier) @local.definition)
(service_definition "service" . (identifier) @local.definition)
(interaction_definition "interaction" . (identifier) @local.definition)
(typedef_identifier) @local.definition
