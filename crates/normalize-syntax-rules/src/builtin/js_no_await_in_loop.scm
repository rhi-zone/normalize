# ---
# id = "js/no-await-in-loop"
# severity = "info"
# tags = ["performance"]
# message = "`await` inside loop - consider using `Promise.all()` for concurrent execution"
# languages = ["javascript", "typescript", "tsx", "jsx"]
# enabled = false
# ---
#
# Using `await` inside a `for`, `for...of`, `for...in`, or `while` loop causes
# each iteration to wait for the previous promise to resolve before starting the
# next one. This serializes operations that could run concurrently, often making
# the code much slower than necessary.
#
# ## How to fix
#
# Collect the promises and await them all at once with `Promise.all`:
#
# ```js
# // Before (serial):
# for (const item of items) {
#   await processItem(item);
# }
#
# // After (concurrent):
# await Promise.all(items.map(item => processItem(item)));
# ```
#
# ## When to disable
#
# This rule is disabled by default (info severity). Sequential awaiting is
# sometimes intentional — for example, when each iteration depends on the
# result of the previous, or when you need to avoid overwhelming an external
# resource with concurrent requests. Disable per site when the sequential
# behavior is required.

; Detects: await expression directly inside a for/while loop body (statement block)
(for_in_statement
  body: (statement_block
    (expression_statement (await_expression) @_await))) @match

(while_statement
  body: (statement_block
    (expression_statement (await_expression) @_await))) @match
