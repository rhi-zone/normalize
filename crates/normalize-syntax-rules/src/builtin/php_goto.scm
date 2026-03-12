# ---
# id = "php/goto"
# severity = "warning"
# tags = ["readability", "maintainability"]
# message = "goto statement makes control flow harder to follow"
# languages = ["php"]
# ---
#
# `goto` makes control flow non-linear and harder to reason about.
# Use structured alternatives: loops with `break`/`continue`, functions,
# or exceptions for error handling.
#
# ## How to fix
#
# ```php
# // Before
# goto retry;
# // After
# while ($shouldRetry) { ... }
# ```

(goto_statement) @match
