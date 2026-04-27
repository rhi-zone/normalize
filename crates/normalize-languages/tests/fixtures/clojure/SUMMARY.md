# normalize-languages/tests/fixtures/clojure

Sample Clojure source file used by `query_fixtures.rs` to exercise all five query types (tags, calls, complexity, imports, types).

- `sample.clj` — representative Clojure program with functions, types, imports, and control flow; includes `^:deprecated` reader metadata on `sum-evens` to exercise `meta_lit` decoration capture.
