# metrics

Metric implementations for the ratchet system. Each metric implements the `Metric` trait (`name()`, `measure_all()`, `higher_is_worse()`).

- `mod.rs` — `Metric` trait definition; re-exports all metric structs
- `complexity.rs` — `ComplexityMetric`: cyclomatic complexity per function via tree-sitter complexity queries, returns `(file/Parent/fn, cc as f64)`
- `call_complexity.rs` — `CallComplexityMetric`: transitive cyclomatic complexity via call-graph BFS; builds call graph from `*.calls.scm` queries, returns `(file/Parent/fn, local_cc + reachable_cc as f64)`
- `file_stats.rs` — `LineCountMetric`, `FunctionCountMetric`, `ClassCountMetric`, `CommentLineCountMetric`: per-file counters using tag queries and comment node traversal
