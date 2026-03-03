; Zig uses PascalCase and UPPER_CASE node names in its grammar.

; Scopes
; ------
; In Zig's grammar, a function declaration is Decl → [FnProto, Block] as siblings.
; Decl wraps both FnProto (signature) and Block (body); marking Decl as a scope
; allows parameters defined inside FnProto to be found from references in Block.
[
  (Decl)
  (Block)
] @local.scope

; Definitions
; -----------

; Function names
(FnProto
  function: (IDENTIFIER) @local.definition)

; Parameters
(ParamDecl
  parameter: (IDENTIFIER) @local.definition)

; Variable and constant declarations
(VarDecl
  variable_type_function: (IDENTIFIER) @local.definition)

; References
; ----------

(IDENTIFIER) @local.reference
