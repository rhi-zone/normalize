# ---
# id = "go/fmt-print"
# severity = "info"
# tags = ["debug-print", "cleanup"]
# message = "fmt.Print found - consider using structured logging"
# languages = ["go"]
# allow = ["**/tests/**", "**/*_test.go", "**/examples/**", "**/cmd/**"]
# enabled = false
# ---
#
# `fmt.Print`, `fmt.Println`, and `fmt.Printf` write directly to stdout,
# which callers of a library cannot suppress or redirect. They produce
# unstructured output with no severity level or context, making them
# unsuitable for production observability.
#
# ## How to fix
#
# Use `log/slog` (Go 1.21+) or `log` for structured, leveled output that
# callers can configure. For library code, consider accepting a logger
# via dependency injection rather than writing to a global.
#
# ## When to disable
#
# `cmd/` entry points and example programs that intentionally write to
# stdout are already excluded in the default allow list. This rule is
# disabled by default (info severity).

((call_expression
  function: (selector_expression
    operand: (identifier) @_pkg
    field: (field_identifier) @_method)
  (#eq? @_pkg "fmt")
  (#any-of? @_method "Print" "Println" "Printf")) @match)
