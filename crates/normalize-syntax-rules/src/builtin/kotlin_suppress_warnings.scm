# ---
# id = "kotlin/suppress-warnings"
# severity = "info"
# tags = ["maintainability"]
# message = "@Suppress annotation found - fix the warning instead"
# languages = ["kotlin"]
# enabled = false
# ---
#
# Warning suppression hides real issues. When possible, fix the
# underlying code rather than suppressing the diagnostic.
#
# ## How to fix
#
# Address the original warning. If suppression is truly necessary,
# add a justification comment.
#
# ## When to disable
#
# Disabled by default. Enable in projects that want to audit warning
# suppressions.

((annotation
  (constructor_invocation
    (user_type
      (type_identifier) @_name))
  (#match? @_name "^Suppress(Warnings)?$")) @match)
