# ---
# id = "python/assert-in-non-test"
# severity = "info"
# tags = ["correctness"]
# message = "`assert` statement can be disabled with `-O`; use explicit validation instead"
# languages = ["python"]
# allow = ["**/tests/**", "**/*_test.py", "**/test_*.py", "**/conftest.py", "**/pytest_*.py"]
# enabled = false
# ---
#
# Python's `assert` statement is removed entirely when running with the `-O`
# (optimize) flag or when `PYTHONOPTIMIZE` is set. Using `assert` for input
# validation, precondition checks, or runtime invariants means those checks
# silently disappear in optimized builds — exactly when you might need them most.
#
# ## How to fix
#
# Replace `assert condition, message` with an explicit raise:
#
# ```python
# if not condition:
#     raise ValueError(f"Expected ..., got {value!r}")
# ```
#
# For preconditions: `ValueError` or `TypeError` are usually appropriate.
# For internal invariants: `RuntimeError` or a custom exception type.
#
# ## When to disable
#
# This rule is disabled by default (info severity). `assert` is idiomatic and
# intentional in test code — use the allow list (default includes common test
# file patterns). For non-test code where you are intentionally relying on
# assert semantics, add a per-line disable comment.

; Detects any assert statement
(assert_statement) @match
