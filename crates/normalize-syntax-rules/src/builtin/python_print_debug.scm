# ---
# id = "python/print-debug"
# severity = "info"
# tags = ["debug-print", "cleanup"]
# message = "print() found - consider using logging module"
# languages = ["python"]
# allow = ["**/tests/**", "**/test_*.py", "**/*_test.py", "**/examples/**", "**/__main__.py"]
# enabled = false
# ---
#
# `print()` in library code is unstructured, cannot be filtered by severity,
# and cannot be suppressed by callers. It mixes debug output with any
# legitimate stdout the application produces, making both harder to
# interpret.
#
# ## How to fix
#
# Use the `logging` module instead. Configure a logger with an appropriate
# name and use `logger.debug()`, `logger.info()`, etc. Callers can then
# control verbosity via log level configuration without modifying library
# code.
#
# ## When to disable
#
# Scripts, CLI entry points, and `__main__.py` files that intentionally write
# to stdout are already excluded in the default allow list. This rule is
# disabled by default (info severity).

((call
  function: (identifier) @_name
  (#eq? @_name "print")) @match)
