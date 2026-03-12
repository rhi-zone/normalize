# ---
# id = "csharp/goto"
# severity = "warning"
# tags = ["readability", "maintainability"]
# message = "goto statement makes control flow harder to follow"
# languages = ["c-sharp"]
# ---
#
# `goto` makes control flow non-linear and harder to reason about.
# Use structured alternatives: loops with `break`/`continue`, methods,
# or state machines.
#
# ## How to fix
#
# ```csharp
# // Before
# goto retry;
# // After
# while (shouldRetry) { ... }
# ```

(goto_statement) @match
