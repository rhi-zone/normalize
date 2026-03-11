# ---
# id = "typescript/no-empty-interface"
# severity = "warning"
# tags = ["style", "correctness"]
# message = "Empty interface `{}` — use a type alias or add members"
# languages = ["typescript", "tsx"]
# enabled = false
# ---
#
# An empty interface (`interface Foo {}`) has no members and provides no
# structural information to the type system. In TypeScript, interfaces and
# type aliases are mostly interchangeable, but empty interfaces are a common
# source of confusion:
#
# - `interface Foo {}` is equivalent to `{}` (the "non-nullish value" type),
#   which accepts almost anything — not what you usually intend.
# - If you want an empty object type, `type Foo = Record<never, never>` is
#   more explicit.
# - If you want to extend an interface to add members later, add a comment
#   explaining that, or add a placeholder member.
# - If you want a marker type (nominal typing), use a branded type or a
#   discriminated union instead.
#
# ## How to fix
#
# - Add the intended members to the interface.
# - Replace with a type alias that expresses intent: `type Foo = {}` or
#   `type Foo = Record<never, never>`.
# - If extending a base interface with no additions, remove the empty
#   interface and use the base directly.
#
# ## When to disable
#
# This rule is disabled by default (warning severity). Empty interfaces used
# as extension points (to be augmented by declaration merging) are a
# legitimate pattern. Disable per file in those contexts.

; Empty interface declaration — no members in the body
(interface_declaration
  body: (interface_body) @_body
  (#match? @_body "^\\{\\s*\\}$")) @match
