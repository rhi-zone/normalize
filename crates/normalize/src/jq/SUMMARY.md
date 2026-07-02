# src/jq

Embedded `jq` drop-in replacement using the `jaq` library (adapted from jaq, MIT; tracks the jaq-core 3.1.0 release API). Exposes `run_jq(args)` as the entry point, invoked either via `normalize jq [args...]` or via a `jq -> normalize` symlink. Submodules: `cli` (argument parsing, mirrors jq's CLI flags), `filter` (jaq filter execution and output formatting). Provides jq-compatible behavior for JSON querying without requiring a separate jq installation.
