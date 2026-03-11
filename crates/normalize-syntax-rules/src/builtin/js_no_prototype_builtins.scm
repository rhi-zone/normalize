# ---
# id = "js/no-prototype-builtins"
# severity = "warning"
# tags = ["correctness", "style"]
# message = "Do not call `Object.prototype` methods directly on objects — use `Object.prototype.X.call(obj, ...)` instead"
# languages = ["javascript", "typescript", "tsx", "jsx"]
# enabled = false
# ---
#
# `Object.prototype` defines several methods (`hasOwnProperty`, `isPrototypeOf`,
# `propertyIsEnumerable`) that are inherited by almost all plain objects.
# Calling them directly — `obj.hasOwnProperty(key)` — looks innocent but has
# two failure modes:
#
# 1. **Object.create(null)** — Objects created with `Object.create(null)` have
#    no prototype chain, so they do not inherit these methods. Calling
#    `obj.hasOwnProperty(key)` on such an object throws a `TypeError`.
#
# 2. **Overridden properties** — An object may define its own `hasOwnProperty`
#    property (intentionally or via user-supplied data), shadowing the
#    prototype method. The shadowed version may have different behaviour,
#    throw, or be a non-function.
#
# ## How to fix
#
# Use the safe form that borrows the method from `Object.prototype` directly:
#
# ```js
# // Bad:
# if (obj.hasOwnProperty(key)) { ... }
#
# // Good:
# if (Object.prototype.hasOwnProperty.call(obj, key)) { ... }
#
# // Modern alternative (ES2022+):
# if (Object.hasOwn(obj, key)) { ... }
# ```
#
# For `isPrototypeOf` and `propertyIsEnumerable`, the same `.call()` pattern
# applies.
#
# ## When to disable
#
# This rule is disabled by default (warning severity). If the codebase
# already guards against prototype-null objects and the call is on a known
# plain object literal, you may disable per site.

; Detects: obj.hasOwnProperty(...), obj.isPrototypeOf(...), obj.propertyIsEnumerable(...)
; Does NOT flag: Object.prototype.hasOwnProperty.call(...) — the callee there has property `call`, not `hasOwnProperty`
(call_expression
  function: (member_expression
    property: (property_identifier) @_method)
  (#match? @_method "^(hasOwnProperty|isPrototypeOf|propertyIsEnumerable)$")) @match
