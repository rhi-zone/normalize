# ---
# id = "js/eq-null"
# severity = "info"
# tags = ["style", "correctness"]
# message = "Consider using `=== null` or `=== undefined` to be explicit"
# languages = ["javascript", "typescript", "tsx", "jsx"]
# enabled = false
# ---
#
# `== null` and `!= null` use loose equality, which matches both `null` and
# `undefined` due to JavaScript's type coercion rules. This is sometimes
# intentional (checking for either absent value in one expression), but the
# ambiguity makes code harder to read and reason about.
#
# ## How to fix
#
# If you intend to check for both `null` and `undefined`:
#
# ```js
# // Make the intent explicit with a comment or a compound check:
# if (x === null || x === undefined) { ... }
# ```
#
# If you only want to check for `null`:
#
# ```js
# if (x === null) { ... }
# ```
#
# ## When to disable
#
# This rule is disabled by default (info severity). Using `== null` to test for
# both null and undefined is a legitimate idiom in some style guides. If your
# team accepts this pattern, disable the rule or add an allow comment.

; Detects: x == null or x != null (loose equality with null)
(binary_expression
  operator: "=="
  right: (null) @_null) @match

(binary_expression
  operator: "!="
  right: (null) @_null) @match
