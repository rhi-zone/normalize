# ---
# id = "csharp/suppress-warnings"
# severity = "info"
# tags = ["maintainability"]
# message = "SuppressMessage / pragma warning disable found - fix the warning instead"
# languages = ["c-sharp"]
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

((preproc_pragma) @match)
