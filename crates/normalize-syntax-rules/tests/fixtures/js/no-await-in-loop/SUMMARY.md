# fixtures/js/no-await-in-loop

Test fixtures for the `js/no-await-in-loop` syntax rule. `match.js` contains `await` expressions directly inside `for...of` and `while` loop bodies. `no_match.js` uses `Promise.all()` for concurrent execution, which should produce no findings.
