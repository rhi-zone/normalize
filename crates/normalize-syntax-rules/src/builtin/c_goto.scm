# ---
# id = "c/goto"
# severity = "info"
# tags = ["readability", "style"]
# message = "goto statement - consider structured control flow"
# languages = ["c", "cpp"]
# enabled = false
# ---
#
# `goto` makes control flow harder to follow and reason about. Most
# uses can be replaced with loops, early returns, or cleanup functions.
# The one common exception is error-cleanup in C (the "goto cleanup"
# pattern), which is idiomatic in kernel and systems code.
#
# ## How to fix
#
# Replace with structured control flow:
# ```c
# // Before
# if (error) goto cleanup;
# // After
# if (error) { cleanup(); return -1; }
# ```
#
# ## When to disable
#
# Disabled by default. The "goto cleanup" pattern is idiomatic in C
# kernel/systems code and should not be flagged there.

((goto_statement) @match)
