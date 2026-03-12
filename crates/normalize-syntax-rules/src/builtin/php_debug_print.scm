# ---
# id = "php/debug-print"
# severity = "info"
# tags = ["debug-print", "cleanup"]
# message = "Debug output function found - consider using a logging framework"
# languages = ["php"]
# allow = ["**/test/**", "**/tests/**", "**/examples/**"]
# enabled = false
# ---
#
# `var_dump`, `print_r`, `var_export`, and `error_log` are debugging
# tools that should not ship in production. Use a PSR-3 compatible
# logger (Monolog, etc.) for structured, level-aware logging.
#
# ## How to fix
#
# ```php
# // Before
# var_dump($user);
# // After
# $this->logger->debug('User data', ['user' => $user]);
# ```
#
# ## When to disable
#
# Disabled by default. Enable if you want to flag debug output in
# production code.

((function_call_expression
  function: (name) @_fn
  (#match? @_fn "^(var_dump|print_r|var_export|error_log|debug_print_backtrace|debug_zval_dump)$")) @match)
