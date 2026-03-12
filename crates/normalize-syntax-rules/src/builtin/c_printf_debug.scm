# ---
# id = "c/printf-debug"
# severity = "info"
# tags = ["debug-print", "cleanup"]
# message = "printf/fprintf found - consider using structured logging"
# languages = ["c", "cpp"]
# allow = ["**/test/**", "**/tests/**", "**/examples/**"]
# enabled = false
# ---
#
# Direct `printf`/`fprintf` calls produce unstructured output that is
# hard to filter and control in production. For libraries especially,
# prefer a logging callback or structured logging framework.
#
# ## How to fix
#
# Replace with your project's logging mechanism:
# ```c
# // Before
# printf("connected to %s:%d\n", host, port);
# // After
# LOG_INFO("connected to %s:%d", host, port);
# ```
#
# ## When to disable
#
# Disabled by default. CLI tools and example programs legitimately
# use printf for user-facing output.

((call_expression
  function: (identifier) @_fn
  (#match? @_fn "^(printf|fprintf|puts|fputs|perror)$")) @match)
