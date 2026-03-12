# ---
# id = "swift/empty-catch"
# severity = "warning"
# tags = ["error-handling", "bug-prone"]
# message = "Empty catch block silently swallows error"
# languages = ["swift"]
# ---
#
# An empty catch block silently discards the error, hiding failures
# that should be logged, rethrown, or handled explicitly. Even if the
# error is intentionally ignored, add a comment explaining why.
#
# ## How to fix
#
# ```swift
# // Before
# catch {}
# // After
# catch {
#     logger.warning("Operation failed: \(error)")
# }
# ```

; Matches catch_block where the text between braces is only whitespace.
((catch_block) @match
 (#match? @match "\\{\\s*\\}$"))
