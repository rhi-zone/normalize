# ---
# id = "java/print-stack-trace"
# severity = "warning"
# tags = ["debug-print", "error-handling"]
# message = "printStackTrace() found - use a logging framework instead"
# languages = ["java"]
# allow = ["**/test/**", "**/tests/**"]
# ---
#
# `Exception.printStackTrace()` writes to stderr without any log level
# or structured context. The output is not captured by logging
# frameworks and cannot be filtered, routed, or correlated.
#
# ## How to fix
#
# ```java
# // Before
# catch (Exception e) { e.printStackTrace(); }
# // After
# catch (Exception e) { logger.error("Operation failed", e); }
# ```

((method_invocation
  name: (identifier) @_method
  (#eq? @_method "printStackTrace")) @match)
