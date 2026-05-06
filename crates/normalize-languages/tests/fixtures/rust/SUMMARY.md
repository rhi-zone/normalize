# fixtures/rust

Rust fixture files for `.scm` query tests and extraction fixtures.

- `sample.rs` — defines a `Counter` struct with `impl` block, a `classify` function with branching, and a `sum_evens` function with a loop; uses `std::collections::HashMap` and `std::fmt`.
- `basic-function/` — extraction fixture: three simple functions (`add`, `greet`, `main`) with `input.rs` + `expected.json`; verifies symbol extraction and call detection.
