# go/context-todo fixture

Fixture files for the `go/context-todo` syntax rule test. `match.go` uses `context.TODO()` as a placeholder in library functions; `no_match.go` uses a properly threaded `ctx context.Context` parameter or `context.Background()` at entry points.
