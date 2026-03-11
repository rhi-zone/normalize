# ---
# id = "python/no-star-import"
# severity = "warning"
# tags = ["style", "maintainability"]
# message = "Wildcard import pollutes namespace — use explicit imports"
# languages = ["python"]
# enabled = false
# ---
#
# `from x import *` imports every public name from module `x` into the current
# namespace. This makes it impossible to tell where a name came from, hides
# dependencies, and risks shadowing local or other imported names.
#
# ```python
# # Bad — unclear where `join` or `exists` come from:
# from os.path import *
#
# # Good — explicit imports are self-documenting:
# from os.path import join, exists
# ```
#
# ## How to fix
#
# Replace the wildcard with explicit names:
# ```python
# from module import name1, name2
# ```
#
# ## When to disable
#
# Wildcard imports are occasionally acceptable in `__init__.py` files that
# re-export a submodule's public API, or in interactive/notebook contexts.
# Disable per-file or per-project as needed.

(import_from_statement
  (wildcard_import)) @match
