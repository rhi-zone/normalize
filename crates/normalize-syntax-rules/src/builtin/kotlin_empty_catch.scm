# ---
# id = "kotlin/empty-catch"
# severity = "warning"
# tags = ["error-handling", "bug-prone"]
# message = "Empty catch block silently swallows exception"
# languages = ["kotlin"]
# ---
#
# An empty catch block silently discards the exception, hiding errors
# that should be logged, rethrown, or handled explicitly. Even if the
# exception is intentionally ignored, add a comment explaining why.
#
# ## How to fix
#
# ```kotlin
# // Before
# catch (e: IOException) {}
# // After
# catch (e: IOException) {
#     logger.warn("IO error (non-fatal)", e)
# }
# ```

; Matches catch_block where the text between braces is only whitespace.
((catch_block) @match
 (#match? @match "\\{\\s*\\}$"))
