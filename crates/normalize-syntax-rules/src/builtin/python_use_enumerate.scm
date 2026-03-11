# ---
# id = "python/use-enumerate"
# severity = "warning"
# tags = ["style", "pythonic"]
# message = "Use `enumerate()` instead of `range(len(...))` to iterate with index and value"
# languages = ["python"]
# enabled = false
# ---
#
# Iterating over indices with `range(len(collection))` to then index into
# the collection is un-idiomatic Python. `enumerate()` provides both the
# index and the value in a single, readable loop:
#
# ```python
# # Un-idiomatic:
# for i in range(len(items)):
#     print(i, items[i])
#
# # Idiomatic:
# for i, item in enumerate(items):
#     print(i, item)
# ```
#
# `enumerate()` is also more efficient: it avoids the redundant `len()`
# call and the repeated index-based lookup `items[i]`.
#
# ## How to fix
#
# Replace `for i in range(len(x)):` with `for i, value in enumerate(x):`.
# If you only need the index and not the value, `enumerate` is still
# preferred — just use `_` for the unused value:
# `for i, _ in enumerate(x):`.
#
# ## When to disable
#
# This rule is disabled by default (warning severity). There are rare cases
# where you need the index to modify the list in-place and `enumerate`
# would produce a copy concern — but even then `enumerate` usually works.
# Disable per site if the loop genuinely needs only the index.

; for i in range(len(collection)) — use enumerate() instead
; Anchors ensure range() is called with exactly one argument (the len() call)
(for_statement
  right: (call
    function: (identifier) @_range
    arguments: (argument_list . (call
      function: (identifier) @_len) .))
  (#eq? @_range "range")
  (#eq? @_len "len")) @match
