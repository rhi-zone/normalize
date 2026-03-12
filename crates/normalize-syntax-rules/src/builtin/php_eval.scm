# ---
# id = "php/eval"
# severity = "error"
# tags = ["security", "bug-prone"]
# message = "eval() executes arbitrary code - security risk"
# languages = ["php"]
# ---
#
# `eval()` interprets a string as PHP code at runtime, enabling code
# injection attacks if any part of the string comes from user input.
# Almost all uses can be replaced with safer alternatives.
#
# ## How to fix
#
# Use arrays/maps for dynamic dispatch, closures for dynamic behavior,
# or template engines for dynamic output:
# ```php
# // Before
# eval('$result = ' . $expression . ';');
# // After
# $result = $calculator->evaluate($expression);
# ```

((function_call_expression
  function: (name) @_fn
  (#eq? @_fn "eval")) @match)
