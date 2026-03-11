# ---
# id = "typescript/no-non-null-assertion"
# severity = "warning"
# tags = ["style", "correctness"]
# message = "Non-null assertion `!` suppresses null checks — use proper narrowing instead"
# languages = ["typescript", "tsx"]
# enabled = false
# ---
#
# The non-null assertion operator `!` tells TypeScript's compiler to treat
# a value as non-null/non-undefined without any runtime check. It is a
# type-level escape hatch that silences the compiler while leaving the code
# vulnerable to `null` or `undefined` dereference at runtime.
#
# ```typescript
# const el = document.getElementById("app")!.innerHTML;
# //                                        ^ crash if element not found
# ```
#
# ## How to fix
#
# Use explicit narrowing instead:
#
# ```typescript
# const el = document.getElementById("app");
# if (el === null) throw new Error("element #app not found");
# el.innerHTML = "<p>hello</p>";
# ```
#
# Or with optional chaining where absence is acceptable:
#
# ```typescript
# const text = document.getElementById("app")?.innerHTML ?? "";
# ```
#
# ## When to disable
#
# This rule is disabled by default (warning severity). In test files or
# in code where the non-null invariant is genuinely guaranteed by surrounding
# logic and adding a runtime check would be noise, use an allow comment.

; Non-null assertion operator `value!`
(non_null_expression) @match
