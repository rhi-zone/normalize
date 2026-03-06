# fixtures/cobra

Captured help output and example program for Go's `cobra` framework (spf13/cobra).

Contains `main.go`, `go.mod`, `go.sum`, `example` (compiled binary), and `example.help` (captured `--help` output). Used by `tests/cobra_fixtures.rs` to verify that `CobraFormat` correctly parses cobra-style help text, which has a characteristic `Available Commands:` section and cobra-specific flag formatting.
