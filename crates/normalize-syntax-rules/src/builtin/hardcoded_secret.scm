# ---
# id = "hardcoded-secret"
# severity = "error"
# tags = ["security"]
# message = "Potential hardcoded secret - use environment variables or config"
# languages = ["rust"]
# allow = ["**/tests/**", "**/examples/**", "**/*.md"]
# ---
#
# Hardcoded secrets — passwords, API keys, tokens, credentials — committed
# to version control are permanently exposed in the repository history. They
# are also likely to appear in logs, error messages, and stack traces, and
# are frequently scanned by automated tools.
#
# ## How to fix
#
# Read secrets from environment variables or a secrets manager at runtime.
# For local development, use a .env file excluded from version control.
# Replace the hardcoded value with a variable lookup.
#
# ## When to disable
#
# Test files that use clearly fake or dummy values (e.g., "test-secret",
# "dummy-key") often trigger this rule. Test directories are already in the
# default allow list; for other false positives, add the file or line to the
# allow list.

; Detects: let password = "..."; let api_key = "..."; etc
; High false positive rate expected - users should allowlist as needed
((let_declaration
  pattern: (identifier) @_name
  value: (string_literal) @_value
  (#match? @_name "(?i)password|secret|api.?key|token|credential")
  (#not-match? @_value "^\"\"$")) @match)
