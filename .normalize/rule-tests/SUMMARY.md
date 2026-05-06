# .normalize/rule-tests

Fixture-based tests for normalize rules. Each case is a pair of files:

- `<case-name>.input.<ext>` — source input file to run rules against
- `<case-name>.expected.json` — expected findings as a JSON array

Run all fixtures: `normalize rules test-fixtures`
Bootstrap/update expected.json: `normalize rules test-fixtures --update`

**Format of expected.json:**
```json
[
  {
    "rule": "rule-id",
    "file": "case-name.input.rs",
    "line": 4,
    "message": "substring of the expected diagnostic message"
  }
]
```

The `message` field uses substring matching — the actual diagnostic message only needs to *contain* the expected string.

**Multi-file cases** use a subdirectory with `input/` + `expected.json` instead.

Current fixtures:
- `no-todo-comment` — verifies the `no-todo-comment` rule fires on `// TODO:` comments
- `rust-unwrap-in-impl` — verifies `rust/unwrap-in-impl` fires on `.unwrap()` calls
