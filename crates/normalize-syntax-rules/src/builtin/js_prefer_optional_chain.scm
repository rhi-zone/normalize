# ---
# id = "js/prefer-optional-chain"
# severity = "info"
# tags = ["style", "readability"]
# message = "Use optional chaining `?.` instead of `&&` guard"
# languages = ["javascript", "typescript", "tsx", "jsx"]
# enabled = false
# ---
#
# JavaScript's optional chaining operator (`?.`) provides a concise, readable
# way to access a chain of properties or call a method only when each step is
# non-null/undefined. The manual `&&`-guard pattern is more verbose and easier
# to get wrong (forgetting a step, wrong variable name, etc.).
#
# ## Patterns flagged
#
# ```js
# // Guard then access — use optional chaining:
# x && x.y          // → x?.y
# x && x.y()        // → x?.y()
# x && x.y.z        // → x?.y.z
# ```
#
# ## How to fix
#
# Replace the `&&`-guard with `?.`:
#
# ```js
# // Before:
# const name = user && user.profile && user.profile.name;
#
# // After:
# const name = user?.profile?.name;
# ```
#
# For calls:
# ```js
# // Before:
# callback && callback();
#
# // After:
# callback?.();
# ```
#
# ## When to disable
#
# This rule is disabled by default (info severity). Optional chaining
# returns `undefined` on a nullish step; if your guard intentionally
# short-circuits to `false`/`null`/`0`, the `&&` form has different
# semantics and should be kept. Also, optional chaining requires ES2020+
# or a transpiler. Disable for codebases targeting older environments.

; x && x.prop — guarded member access on same identifier
(binary_expression
  left: (identifier) @_base
  operator: "&&"
  right: (member_expression
    object: (identifier) @_obj)
  (#eq? @_base @_obj)) @match

; x && x.prop() — guarded method call on same identifier
(binary_expression
  left: (identifier) @_base
  operator: "&&"
  right: (call_expression
    function: (member_expression
      object: (identifier) @_obj))
  (#eq? @_base @_obj)) @match

; x && x() — guarded call of same identifier
(binary_expression
  left: (identifier) @_base
  operator: "&&"
  right: (call_expression
    function: (identifier) @_fn)
  (#eq? @_base @_fn)) @match
