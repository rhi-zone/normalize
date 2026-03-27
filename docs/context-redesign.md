# Context Redesign

Replaces the current `normalize context` (hierarchical `.context.md` walk) with a
general-purpose contextual text resolution system.

## Problem

The current `normalize context` walks `.context.md` files up the directory tree and
concatenates them. This is ad-hoc: no filtering, no conditions, no metadata, single
file per directory.

We need a system that:

- Resolves text from hierarchical sources with arbitrary metadata
- Filters by matching caller-provided context against source metadata
- Supports composable conditions (`any`/`all`) with pluggable match strategies
- Is general — not specific to any consumer (Claude Code, editors, CI, etc.)
- Is fast — with a daemon/file-watcher upgrade path

## Data Model

### Source files

Markdown files with YAML frontmatter. Live in `.normalize/context/` directories,
walked hierarchically (project → parent → ... → global `~/.normalize/context/`).

Users split files however they want — one per hint, one per category, one big file
with `---`-separated blocks. The system scans all `.md` files in each context directory.

```markdown
---
claudecode:
  hook: UserPromptSubmit
  matcher: Read
scope:
  language: rust
  size: large
---
Before reading large files, try `normalize view <path>` for a structural outline.
Then read only the sections you need.
```

Frontmatter is arbitrary nested YAML. No reserved field names. The structure is
whatever the author and consumer agree on.

Multiple blocks per file are separated by `---`:

```markdown
---
claudecode:
  hook: UserPromptSubmit
scope:
  language: rust
---
Prefer `cargo test -q` over `cargo test`.

---
ci:
  stage: pre-merge
scope:
  language: rust
---
Run `cargo clippy -- -D warnings` before merging.
```

### Context (caller-provided)

The caller provides a flat or nested map of key-value pairs representing the current
context. This is what source metadata is matched against.

```bash
# CLI: dot-path key=value pairs
normalize context --match claudecode.hook=UserPromptSubmit --match scope.language=rust

# Stdin: JSON object, optionally prefixed into a namespace
echo '{"hook":"UserPromptSubmit","tool_name":"Read"}' \
  | normalize context --stdin --prefix claudecode

# Dump all (no filtering)
normalize context --all
```

Dot-paths in `--match` address into the nested frontmatter structure.

### Conditions

When frontmatter contains a `conditions:` block, it defines richer matching rules
instead of simple key equality.

```yaml
---
conditions:
  all:
    - claudecode.hook:
        equals: UserPromptSubmit
    - prompt:
        keywords: [read, file, explore]
  any:
    - scope.language:
        equals: rust
    - scope.language:
        equals: go
---
```

- `all:` — every condition must pass (default for bare `conditions:`)
- `any:` — at least one must pass
- Nestable: `any:` can contain `all:` groups and vice versa

Each condition is `{field: {strategy: args}}`. A bare value is shorthand for `equals`.

### Match Strategies

Each strategy is a named evaluator. Built-in:

| Strategy | Args | Behavior |
|----------|------|----------|
| `equals` | string | Exact match (default for bare values) |
| `contains` | string | Substring match |
| `keywords` | string[] | Any keyword appears as substring |
| `regex` | string | Regex match |
| `exists` | bool | Field exists (or doesn't) in caller context |
| `one_of` | string[] | Value is one of the listed options |

Adding a new strategy = implementing a new evaluator function. Plugin-shaped:
the set of strategies is extensible.

### Matching without conditions

If a source file has no `conditions:` block, its non-reserved frontmatter keys are
matched using `equals` against the caller's context. Only keys present in both the
source and the caller's context are compared (missing keys don't fail).

```yaml
---
claudecode:
  hook: UserPromptSubmit
---
This matches if the caller provides claudecode.hook=UserPromptSubmit.
Other caller context keys are ignored.
```

## CLI Interface

```
normalize context [OPTIONS]

Options:
    --match KEY=VALUE      Match context against this key-value pair (repeatable)
    --stdin                Read context as JSON from stdin
    --prefix PREFIX        Namespace stdin JSON under this prefix
    --all                  Return all context entries (no filtering)
    --from PATH            Override context directory name (default: context)
    --root PATH            Root directory for hierarchy walk (default: cwd)
    --list                 Show source file paths only, not content
    --pretty / --compact   Output formatting
    --json / --jsonl       Machine-readable output
```

### Output

Default: concatenated text bodies of matching blocks, separated by newlines.

With `--json`: array of objects with `source` (file path), `metadata` (frontmatter),
and `body` (text content).

### Examples

```bash
# Claude Code hook shim (entire hook script):
cat | normalize context --stdin --prefix claudecode

# Editor plugin querying for language-specific hints:
normalize context --match scope.language=rust --match editor.event=save

# CI checking for pre-merge guidance:
normalize context --match ci.stage=pre-merge

# List all context sources in the hierarchy:
normalize context --all --list
```

## Directory Hierarchy

Walked bottom-up from the working directory:

```
~/.normalize/context/              # global
~/git/rhizone/.normalize/context/  # org
~/git/rhizone/normalize/.normalize/context/  # project
```

Configurable directory name via `--from` or `.normalize/config.toml`:

```toml
[context]
directory = "context"   # looked up as .normalize/{directory}/
```

## Performance

### v1: Scan on every call

Walk directories, parse frontmatter, evaluate conditions. For a typical hierarchy
(3-5 directories, <50 files total), this is <50ms.

### v2: Daemon with file watcher

The normalize daemon (already exists) pre-indexes all context files in memory.
`normalize context` queries the daemon via unix socket. Near-zero latency.
File changes detected via inotify/fsevents, index updated incrementally.

### v3: Embedding search

Each context block gets an embedding. `normalize context --semantic "query text"`
returns top-k by similarity. Natural extension of the daemon's in-memory index.

## Migration from .context.md

The current `.context.md` files are simple markdown with no frontmatter. Migration:

1. Move content into `.normalize/context/` as `.md` files
2. Add frontmatter if filtering is needed (optional — bare files always match)
3. Remove old `.context.md` files
4. Update `normalize context` to use new system (old behavior available via flag
   during transition, then removed)
