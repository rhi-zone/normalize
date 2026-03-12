# ---
# id = "csharp/thread-sleep"
# severity = "warning"
# tags = ["bug-prone", "correctness"]
# message = "Thread.Sleep() blocks the current thread - consider async alternatives"
# languages = ["c-sharp"]
# allow = ["**/test/**", "**/tests/**"]
# enabled = false
# ---
#
# `Thread.Sleep()` blocks the current thread and is rarely correct in
# modern C# code. Use `Task.Delay` with `await` for non-blocking waits.
#
# ## How to fix
#
# ```csharp
# // Before
# Thread.Sleep(1000);
# // After
# await Task.Delay(1000);
# ```
#
# ## When to disable
#
# Disabled by default. Test code is already excluded. Enable if you
# want to flag accidental sleeps in production code.

((invocation_expression
  function: (member_access_expression
    expression: (identifier) @_obj
    name: (identifier) @_method)
  (#eq? @_obj "Thread")
  (#eq? @_method "Sleep")) @match)
