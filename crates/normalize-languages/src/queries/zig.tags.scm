; Zig tags query
; Covers: function declarations, container declarations (structs/enums/unions)
;
; Zig's tree-sitter grammar uses PascalCase node names.
; FnProto has field: function (IDENTIFIER)
; VarDecl has field: variable_type_function (IDENTIFIER)

; Function declarations: pub fn add(a: i32, b: i32) i32 { ... }
(FnProto
  function: (IDENTIFIER) @name) @definition.function

; Container declarations (struct, enum, union) referenced via VarDecl
; In Zig, types are values: `const Foo = struct { ... }`
; The VarDecl holds the name; ContainerDecl is the type expression.
(VarDecl
  variable_type_function: (IDENTIFIER) @name
  (ContainerDecl)) @definition.class
