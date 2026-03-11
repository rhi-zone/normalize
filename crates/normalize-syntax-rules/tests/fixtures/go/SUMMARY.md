# fixtures/go

Test fixtures for Go-language syntax rules.

Each subdirectory corresponds to one builtin Go rule and contains a `match.go` file (expected to produce findings) and a `no_match.go` file (expected to produce zero findings). Rules covered: `go/fmt-print`, `go/many-returns`, `go/package-var`, `go/error-ignored`, `go/empty-return`, `go/defer-in-loop`, `go/context-todo`, `go/sync-mutex-copied`.
