# ---
# id = "swift/thread-sleep"
# severity = "warning"
# tags = ["bug-prone", "correctness"]
# message = "Thread.sleep blocks the current thread - consider Task.sleep for async code"
# languages = ["swift"]
# allow = ["**/test/**", "**/tests/**"]
# enabled = false
# ---
#
# `Thread.sleep` blocks the current thread. In Swift concurrency,
# prefer `Task.sleep` for non-blocking suspension.
#
# ## How to fix
#
# ```swift
# // Before
# Thread.sleep(forTimeInterval: 1.0)
# // After
# try await Task.sleep(nanoseconds: 1_000_000_000)
# ```
#
# ## When to disable
#
# Disabled by default. Test code is already excluded. Enable if you
# want to flag accidental sleeps in production code.

((call_expression
  (navigation_expression
    (simple_identifier) @_obj
    (navigation_suffix
      (simple_identifier) @_method))
  (#eq? @_obj "Thread")
  (#eq? @_method "sleep")) @match)
