# ---
# id = "csharp/empty-catch"
# severity = "warning"
# tags = ["error-handling", "bug-prone"]
# message = "Empty catch block silently swallows exception"
# languages = ["c-sharp"]
# recommended = true
# ---
#
# An empty catch block silently discards the exception, hiding errors
# that should be logged, rethrown, or handled explicitly. Even if the
# exception is intentionally ignored, add a comment explaining why.
#
# ## How to fix
#
# ```csharp
# // Before
# catch (IOException e) {}
# // After
# catch (IOException e) {
#     _logger.LogWarning(e, "IO error (non-fatal)");
# }
# ```

((catch_clause
  body: (block) @_body
  (#eq? @_body "{ }")) @match)
