# ---
# id = "php/empty-catch"
# severity = "warning"
# tags = ["error-handling", "bug-prone"]
# message = "Empty catch block silently swallows exception"
# languages = ["php"]
# ---
#
# An empty catch block silently discards the exception, hiding errors
# that should be logged, rethrown, or handled explicitly. Even if the
# exception is intentionally ignored, add a comment explaining why.
#
# ## How to fix
#
# ```php
# // Before
# catch (Exception $e) {}
# // After
# catch (Exception $e) {
#     $this->logger->warning('Operation failed', ['exception' => $e]);
# }
# ```

((catch_clause
  body: (compound_statement) @_body
  (#eq? @_body "{ }")) @match)
