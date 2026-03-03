; Source: arborium (tree-sitter-starlark). Convention converted from @scope/@definition/@reference
; to @local.scope/@local.definition/@local.reference. #set! predicates stripped (Neovim-specific).

; Scopes
(module) @local.scope

(function_definition
  name: (identifier) @local.definition) @local.scope

(dictionary_comprehension) @local.scope
(list_comprehension) @local.scope

; Definitions

; Function parameters
(parameters
  (parameter
    (identifier) @local.definition))

(parameters
  (parameter
    (default_parameter
      name: (identifier) @local.definition)))

; *args
(parameters
  (parameter
    (list_splat_pattern
      (identifier) @local.definition)))

; **kwargs
(parameters
  (parameter
    (dictionary_splat_pattern
      (identifier) @local.definition)))

; Loop variables
(for_statement
  left: (identifier) @local.definition)

(for_statement
  left: (tuple_pattern
    (identifier) @local.definition))

(for_in_clause
  left: (identifier) @local.definition)

(for_in_clause
  left: (tuple_pattern
    (identifier) @local.definition))

; Assignments
(assignment
  left: (identifier) @local.definition)

(assignment
  left: (tuple_pattern
    (identifier) @local.definition))

; References
(identifier) @local.reference
