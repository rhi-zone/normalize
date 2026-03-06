# ---
# id = "python/bare-except"
# severity = "warning"
# tags = ["error-handling"]
# message = "Bare `except:` catches all exceptions including SystemExit and KeyboardInterrupt"
# languages = ["python"]
# enabled = false
# ---
#
# A bare `except:` clause without a specific exception type catches every
# exception that Python can raise, including `SystemExit` (raised by
# `sys.exit()`), `KeyboardInterrupt` (raised by Ctrl+C), and
# `GeneratorExit`. Swallowing these prevents clean shutdown of the program,
# makes it impossible to interrupt with Ctrl+C, and silently hides bugs that
# should propagate.
#
# It also catches programming errors (`TypeError`, `AttributeError`, etc.)
# that are usually symptoms of bugs rather than recoverable runtime conditions.
# Catching and silencing them makes debugging much harder.
#
# ## How to fix
#
# Catch the specific exception types you expect and intend to handle:
#
# ```python
# try:
#     do_something()
# except ValueError as e:
#     handle(e)
# ```
#
# If you genuinely need to catch all application-level exceptions, use
# `except Exception:` — this still allows `SystemExit`, `KeyboardInterrupt`,
# and `GeneratorExit` to propagate.
#
# ## When to disable
#
# This rule is disabled by default (warning severity). Bare `except:` is
# occasionally used in top-level crash reporters that must not re-raise under
# any circumstances. Disable per site with an allow comment.

; Detects bare except: clauses with no exception type specified
; The . anchors ensure there is no node between "except" and ":" (no type present)
(except_clause . "except" . ":") @match
