# ---
# id = "java/empty-catch"
# severity = "warning"
# tags = ["error-handling", "bug-prone"]
# message = "Empty catch block silently swallows exception"
# languages = ["java"]
# ---
#
# An empty catch block silently discards the exception, hiding errors
# that should be logged, rethrown, or handled explicitly. Even if the
# exception is intentionally ignored, add a comment explaining why.
#
# ## How to fix
#
# Log the exception, rethrow it, or add a comment:
# ```java
# // Before
# catch (IOException e) {}
# // After
# catch (IOException e) {
#     logger.warn("IO error (non-fatal)", e);
# }
# ```

((catch_clause
  body: (block) @_body
  (#eq? @_body "{}")) @match)
