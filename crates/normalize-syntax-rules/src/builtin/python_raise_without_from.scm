# ---
# id = "python/raise-without-from"
# severity = "warning"
# tags = ["correctness", "error-handling"]
# message = "Raise inside `except` block should chain the cause: `raise X from e` preserves the original traceback"
# languages = ["python"]
# enabled = false
# ---
#
# When raising a new exception inside an `except` block, Python 3 allows
# chaining exceptions with `raise NewException(...) from original`. This
# preserves the original traceback and clearly signals the causal relationship
# between the two exceptions.
#
# Without `from`, Python 3 still implicitly chains the exception context
# (visible as "During handling of the above exception, another exception
# occurred"), but using `from e` makes the chain explicit and intentional.
# Using `raise X from None` explicitly suppresses chaining when desired.
#
# ```python
# # Loses explicit causal chain:
# except ValueError:
#     raise RuntimeError("conversion failed")
#
# # Preserves causal chain:
# except ValueError as e:
#     raise RuntimeError("conversion failed") from e
#
# # Explicitly suppresses chain:
# except ValueError:
#     raise RuntimeError("conversion failed") from None
# ```
#
# ## How to fix
#
# Add `from e` to the raise statement, where `e` is the exception variable
# from the `except` clause:
# ```python
# except ValueError as e:
#     raise RuntimeError("msg") from e
# ```
#
# To explicitly suppress context: `raise X from None`.
#
# ## When to disable
#
# This rule is disabled by default (warning severity). Exception chaining
# can be intentionally omitted when the new exception is unrelated to the
# original, or when the original should not appear in user-facing output.
# Use `from None` to signal intentional suppression.

; raise X inside except block without from clause
; Matches direct raises (not nested in if/for) that have a value but no cause
(except_clause
  (block
    (raise_statement
      (_)
      !cause) @match))
