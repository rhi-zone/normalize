# ---
# id = "python/mutable-default-arg"
# severity = "warning"
# tags = ["bug-prone"]
# message = "Mutable default argument - shared across calls, use `None` as default"
# languages = ["python"]
# enabled = false
# ---
#
# Python evaluates default argument values once at function definition time,
# not each time the function is called. When the default is a mutable object
# (a list or dict), all callers share the same object. Mutations made inside
# the function persist across calls, accumulating state invisibly.
#
# This is a classic Python gotcha that produces bugs that are hard to reproduce
# because the function behaves differently depending on how many times it has
# been called before.
#
# ```python
# def append(val, lst=[]):
#     lst.append(val)
#     return lst
#
# append(1)  # [1]
# append(2)  # [1, 2]  ← the list was shared!
# ```
#
# ## How to fix
#
# Use `None` as the default and create the mutable object inside the function:
#
# ```python
# def append(val, lst=None):
#     if lst is None:
#         lst = []
#     lst.append(val)
#     return lst
# ```
#
# ## When to disable
#
# This rule is disabled by default (warning severity). In rare cases, sharing a
# mutable default is intentional (e.g., a simple memoization cache). Disable
# per site with an allow comment and a clear explanation.

; Detects def f(x=[]) or def f(x={}) — mutable list or dict as default argument value
(default_parameter
  value: [(list) (dictionary)] @_val) @match
