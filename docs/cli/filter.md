# normalize filter

Filter files by glob patterns and inspect the `@aliases` used by `--exclude` / `--only`.

Owned by the `normalize-filter` crate (CLI surface behind its `cli` feature).

> The old top-level `normalize aliases` is kept as a **hidden transitional alias** for
> `normalize filter aliases` for one release. Migrate to `filter aliases`.

## Subcommands

### `filter aliases`

List built-in and config-defined filter aliases, resolved for the project's detected
languages.

```bash
normalize filter aliases              # List all aliases
normalize filter aliases --json       # JSON output
normalize filter aliases --root <DIR> # Specify project root
```

### `filter matches`

Check whether a path passes a set of `--exclude` / `--only` filters (aliases resolved).

```bash
normalize filter matches src/main.rs --only "*.rs"
normalize filter matches foo_test.go --exclude @tests
normalize filter matches path/to/file --only @docs --root <DIR>
```

## Builtin Aliases

| Alias | Description |
|-------|-------------|
| `@tests` | Test files and directories (language-aware) |
| `@config` | Configuration files |
| `@build` | Build output directories |
| `@docs` | Documentation files |
| `@generated` | Generated code |

## Custom Aliases

Define in `.normalize/config.toml`:

```toml
[aliases]
tests = ["*_test.go", "**/__tests__/**"]
config = ["*.toml", "*.yaml", "*.json"]
todo = ["TODO.md", "TASKS.md"]
```

Set patterns to an empty array to disable a builtin alias:

```toml
[aliases]
generated = []  # Disable @generated
```

## Usage with Other Commands

```bash
normalize view . --exclude @tests
normalize analyze --only @config
normalize grep "TODO" --exclude @generated
```
