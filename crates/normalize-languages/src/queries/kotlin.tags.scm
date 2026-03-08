; Vendored from https://github.com/fwcd/tree-sitter-kotlin
; License: MIT

; Classes
(class_declaration
  (type_identifier) @name) @definition.class

; Objects
(object_declaration
  (type_identifier) @name) @definition.class

; Functions (top-level and member)
(function_declaration
  (simple_identifier) @name) @definition.function

; Properties (class-level only via class body filter)
; NOTE: property_declaration is intentionally omitted here because the Kotlin
; grammar uses the same node kind for class-level properties AND local val/var
; declarations inside function bodies. The extraction layer has no way to
; distinguish them without ancestor traversal, and including them causes all
; symbols to be silently dropped (the first property_declaration with an
; un-resolvable name causes collect_symbols_from_tags to return None).

; Enum entries
(enum_entry
  (simple_identifier) @name) @definition.constant

; Type aliases
(type_alias
  (type_identifier) @name) @definition.type

; Companion objects (only named ones)
(companion_object
  (type_identifier) @name) @definition.class

; Function calls
(call_expression
  (simple_identifier) @name) @reference.call

; Method calls via navigation
(call_expression
  (navigation_expression
    (navigation_suffix
      (simple_identifier) @name))) @reference.call

; Constructor invocations (class references)
(constructor_invocation
  (user_type
    (type_identifier) @name)) @reference.class
