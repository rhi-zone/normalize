; Zig tags query
; Covers: function declarations, container declarations (structs/enums/unions)
;
; Zig's tree-sitter grammar uses PascalCase node names.

; Function declarations
(FnProto
  name: (IDENTIFIER) @name) @definition.function

; Test declarations
(TestDecl
  name: (STRINGLITERALSINGLE) @name) @definition.function

; Container declarations (struct, enum, union) referenced via VarDecl
; In Zig, types are values: `const Foo = struct { ... }`
; The VarDecl holds the name; ContainerDecl is the type expression.
(VarDecl
  name: (IDENTIFIER) @name
  (ContainerDecl)) @definition.class
