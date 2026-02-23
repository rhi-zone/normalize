# ---
# id = "rust/unnecessary-type-alias"
# severity = "info"
# tags = ["style"]
# message = "Type alias to simple type - consider using the type directly"
# languages = ["rust"]
# enabled = false
# ---
#
# `type Foo = Bar;` without generics or semantic distinction adds a layer of
# indirection: callers see `Foo` in signatures and must look up the alias to
# understand what type they are actually working with. Without adding
# semantics or encapsulation, the alias is pure noise.
#
# ## How to fix
#
# Use the underlying type directly. If the alias adds domain meaning (e.g.,
# `type UserId = String`) or is used for re-exporting in a public API,
# the alias is intentional — disable per file.
#
# ## When to disable
#
# This rule is disabled by default (info severity). Semantic newtype aliases
# and public re-exports are legitimate uses.

; Detects: type X = Y; where both are simple type identifiers
; May be intentional for re-exports or semantic clarity
(type_item
  name: (type_identifier) @_alias
  type: (type_identifier) @_target) @match
