# ---
# id = "python/use-with"
# severity = "warning"
# tags = ["correctness", "resources"]
# message = "Use 'with open(...)' to ensure the file is closed — bare open() may leak handles"
# languages = ["python"]
# enabled = true
# recommended = true
# ---
#
# Assigning the result of `open()` directly to a variable risks leaking the
# file handle if the code path doesn't explicitly call `.close()`, or if an
# exception is raised before the close.
#
# ```python
# # Bad:
# f = open("data.txt")
# data = f.read()
# f.close()  # skipped on exception
#
# # Good:
# with open("data.txt") as f:
#     data = f.read()
# ```
#
# The `with` statement guarantees the file is closed when the block exits,
# even on exceptions.
#
# ## How to fix
#
# Wrap the `open()` call in a `with` statement:
# `with open("file") as f:`.
#
# ## When to disable
#
# Disable when the file handle intentionally outlives the current scope
# (e.g., returned from a factory function) or when using a manual
# try/finally to manage the resource.

(assignment
  right: (call
    function: (identifier) @_fn
    (#eq? @_fn "open"))) @match
