# ---
# id = "java/system-print"
# severity = "info"
# tags = ["debug-print", "cleanup"]
# message = "System.out/err print found - consider using a logging framework"
# languages = ["java"]
# allow = ["**/test/**", "**/tests/**", "**/examples/**"]
# enabled = false
# ---
#
# `System.out.println`, `System.err.println`, and their variants write
# directly to stdout/stderr. In production code, prefer a structured
# logging framework (SLF4J, Log4j2, java.util.logging) that supports
# log levels, structured fields, and configurable output.
#
# ## How to fix
#
# Replace with your project's logging framework:
# ```java
# // Before
# System.out.println("Processing user " + userId);
# // After
# logger.info("Processing user {}", userId);
# ```
#
# ## When to disable
#
# Test code and example programs that intentionally write to stdout
# are already excluded in the default allow list.

((method_invocation
  object: (field_access
    object: (identifier) @_obj
    field: (identifier) @_field)
  name: (identifier) @_method
  (#eq? @_obj "System")
  (#match? @_field "^(out|err)$")
  (#match? @_method "^(println|print|printf|format)$")) @match)
