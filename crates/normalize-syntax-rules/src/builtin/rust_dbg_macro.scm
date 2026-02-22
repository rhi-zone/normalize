# ---
# id = "rust/dbg-macro"
# severity = "warning"
# tags = ["debug-print", "cleanup"]
# message = "dbg!() macro found - remove before committing"
# languages = ["rust"]
# allow = ["**/tests/**"]
# ---
#
# `dbg!()` is a development-only debugging tool that prints the expression,
# its value, and the file/line to stderr, then returns the value. It is
# convenient during development but must never be committed — it adds
# noise to stderr in every environment where the code runs.
#
# ## How to fix
#
# Remove the `dbg!()` call. If you need the expression result, assign it
# to a variable or use it directly. If you need the debug output, use
# `tracing::debug!` or `log::debug!` instead.
#
# ## When to disable
#
# Never — `dbg!()` has no legitimate use in committed code.

((macro_invocation
  macro: (identifier) @_name
  (#eq? @_name "dbg")) @match)
