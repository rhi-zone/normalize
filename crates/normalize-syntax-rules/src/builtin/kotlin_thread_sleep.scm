# ---
# id = "kotlin/thread-sleep"
# severity = "warning"
# tags = ["bug-prone", "correctness"]
# message = "Thread.sleep() blocks the current thread - consider coroutine delay()"
# languages = ["kotlin"]
# allow = ["**/test/**", "**/tests/**"]
# enabled = false
# ---
#
# `Thread.sleep()` blocks the current thread. In Kotlin, prefer
# `delay()` inside a coroutine for non-blocking suspension.
#
# ## How to fix
#
# ```kotlin
# // Before
# Thread.sleep(1000)
# // After
# delay(1000)
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
