# ---
# id = "kotlin/println-debug"
# severity = "info"
# tags = ["debug-print", "cleanup"]
# message = "println/print found - consider using a logging framework"
# languages = ["kotlin"]
# allow = ["**/test/**", "**/tests/**", "**/examples/**"]
# enabled = false
# ---
#
# `println` and `print` write directly to stdout. In production code,
# use a logging framework (SLF4J, Logback, kotlin-logging) for
# structured output and log levels.
#
# ## How to fix
#
# ```kotlin
# // Before
# println("User $id logged in")
# // After
# logger.info { "User $id logged in" }
# ```
#
# ## When to disable
#
# Disabled by default. Enable if you want to flag console output in
# production code. Test and example directories are already excluded.

((call_expression
  (simple_identifier) @_fn
  (#match? @_fn "^(println|print)$")) @match)
