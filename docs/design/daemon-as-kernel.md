# Daemon as Kernel — Design

## Framing

The normalize daemon is the codebase's kernel: it maintains live state that commands
query. Commands should not directly open parsers, databases, or caches. They get
injected APIs that transparently route through daemon, index, or compute-on-miss.

The principle is **"don't expose the mechanism, expose the query."** If the bypass
doesn't exist in the API, it can't be accidentally used.

This is not a new architecture — it's the direction the codebase is already moving.
The `FileRule` trait proved the pattern works: rule authors implement `check_file()`,
the framework handles caching, parallelization, and file-walking. New rules can't
forget caching because they never touch `FindingsCache`. This design extends that
same idea to the rest of normalize.

## What the Daemon Maintains

The daemon's job is incremental maintenance of the codebase model. Four data stores,
each with its own invalidation strategy:

### Symbol index (`index.sqlite`)

On file change: re-extract symbols for that one file, update rows. Currently only
updates on `structure rebuild`; the migration to incremental single-file re-index is
the next step. The index stores symbols, imports, and call edges — the structural
facts that cross-file features (import resolution, dead code, call graphs) need.

### Findings cache (`findings-cache.sqlite`)

On file change: invalidate cached findings for that file. Already built via the
`FileRule` framework. The daemon pre-warms findings eagerly on file-change events
so that `rules run` and `ci` queries return sub-millisecond cached results instead
of seconds of cold evaluation.

### Embeddings (semantic search vectors)

On file change: re-embed changed symbols in the background. Designed in
`docs/design/semantic-retrieval.md`. The daemon keeps embeddings warm so that
`structure search` queries reflect the current state of the codebase without a
manual rebuild.

### NOT full ASTs

Storing full ASTs is too expensive and too volatile. Symbol resolution is the right
granularity — cheap to maintain incrementally, covers the query patterns that matter
(resolve a name, list symbols in a file, find callers). Commands that need the raw
CST (`syntax ast`, `syntax query`) parse on demand; they are the explicit escape
hatch, not the default path.

## Injected APIs

Commands interact with the codebase model through trait APIs, not through database
handles or parser calls. The implementation behind the trait routes to whatever
source is available: daemon socket (warm), SQLite file (cold), or compute-on-miss
(no index).

### `FileRule` (already built)

```rust
pub trait FileRule: Send + Sync {
    type Finding: Serialize + DeserializeOwned + Send;
    fn engine_name(&self) -> &str;
    fn config_hash(&self) -> String;
    fn check_file(&self, path: &Path, root: &Path) -> Vec<Self::Finding>;
    fn to_diagnostics(&self, ...) -> DiagnosticsReport;
}
```

Rule authors implement two methods. The framework (`run_file_rule`) handles cache
lookup, parallel execution, cache storage, and report assembly. The daemon primes
findings eagerly; the CLI retrieves them. Rule code never touches `FindingsCache`.

### `SymbolIndex` (to build)

```rust
pub trait SymbolIndex {
    fn resolve(&self, path: &Path, name: &str) -> Option<Location>;
    fn symbols_in(&self, path: &Path) -> Vec<Symbol>;
    fn callers_of(&self, path: &Path, name: &str) -> Vec<Location>;
    fn imports_of(&self, path: &Path) -> Vec<Import>;
}
```

Commands like `view`, `edit`, `analyze` use this instead of parsing files directly.
The implementation checks daemon first (warm in-memory index), falls back to SQLite
(cold but still fast), falls back to parse-on-demand (no index, single-file only).

### Embedding queries (to build)

```rust
pub trait SemanticSearch {
    fn search(&self, query: &str, limit: usize) -> Vec<Match>;
}
```

The daemon keeps embeddings warm. If unavailable, falls back to cold SQLite vector
search. Exposed as `structure search`.

## Command Taxonomy

Three categories of commands, each with different daemon relationships:

### Query commands

`view`, `edit`, `analyze`, `grep`, `structure query`

These query the codebase model. They go through injected APIs (`SymbolIndex`,
`SemanticSearch`) that transparently use the daemon's warm state. If daemon
unavailable, fall back to direct index query or compute-on-miss. The command code
never knows which path was taken.

### Batch commands

`rules run`, `ci`, `structure rebuild`

These run computations across many files. They use the `FileRule`-style framework
with automatic caching. The daemon pre-warms results on file changes; the CLI
retrieves cached results. `structure rebuild` is the one command that writes to the
index directly — it's the equivalent of a kernel rebuild.

### Raw access commands

`syntax ast`, `syntax query`

These legitimately need the raw parser. They're the escape hatch — explicitly opt
into tree-sitter for inspection and debugging. Small set, clearly marked, not the
default path for any user-facing workflow.

## Daemon Lifecycle

### Auto-start

On first `normalize` invocation that needs daemon state, the CLI spawns the daemon
as a fire-and-forget background process. This does not block the CLI — already
implemented, eliminating the previous 2-second startup overhead. The daemon writes
its PID and socket path to `~/.config/normalize/daemon.lock`.

### File watching

inotify (Linux) / FSEvents (macOS), debounced. On file change:

1. Invalidate findings cache for that file
2. Re-extract symbols for that file (incremental index update)
3. Queue re-embedding if embeddings are enabled
4. Broadcast event to subscribers (`Subscribe` protocol)

Each step is independent and can fail without blocking the others.

### Idle management

After N minutes of no file changes, release memory (drop parsed state, shrink
caches). Re-wake on next change event or CLI query. The daemon process stays alive
but dormant — restarting has higher latency than waking from idle.

### Resource budget

Background threads run at low priority (`nice +10`). Working set capped: the daemon
tracks memory usage and evicts least-recently-used entries when it exceeds the
budget. This is essential — a daemon that grows to consume 2GB defeats the purpose
of a lightweight tool.

## Protocol Evolution

The current daemon protocol has 7 request types:

| Request | Purpose | Status |
|---------|---------|--------|
| `Add` | Register a root for watching | Built |
| `Remove` | Unregister a root | Built |
| `List` | List watched roots | Built |
| `Status` | Daemon health/uptime | Built |
| `Shutdown` | Graceful stop | Built |
| `Subscribe` | Stream file-change events | Built |
| `RunRules` | Query cached diagnostics | Built |

The direction is to add query-oriented requests:

| Request | Purpose | Status |
|---------|---------|--------|
| `QueryIndex(sql)` | Direct index queries | Planned |
| `ResolveSymbol(path, name)` | Symbol lookups | Planned |
| `SearchEmbeddings(query, limit)` | Semantic search | Planned |

Commands don't know about the protocol. They use trait APIs (`SymbolIndex`,
`SemanticSearch`); the implementation routes to the daemon socket transparently.
The protocol is an implementation detail of the trait backends, not a user-facing
API.

The existing enum + handler pattern (`Request` enum, `handle_request` dispatcher)
is the right structure — adding a variant and a handler method is mechanical.

## Migration Path

Ordered by dependency and value. Each step is independently shippable.

### 1. `FileRule` trait makes rule caching automatic — Done

Rule authors implement `check_file()`, framework handles everything else. Daemon
primes findings eagerly on file changes. This proved the pattern.

### 2. Daemon fire-and-forget spawn — Done

`normalize` invocations that need the daemon start it without blocking. Eliminated
the 2-second startup overhead that made daemon usage feel expensive.

### 3. Daemon incremental index update on file change — Next

Single-file re-index when the file watcher fires. Currently `structure rebuild`
re-indexes everything; the incremental path re-extracts symbols for one file and
updates the corresponding rows. This makes the index live rather than stale-until-
rebuild.

### 4. Wire `rules run` / `ci` to daemon's pre-warmed findings cache — Next

The daemon already primes findings eagerly. The CLI needs to prefer the daemon's
cache over cold evaluation when the daemon is available. Fallback to direct
`run_file_rule` when daemon is down.

### 5. `SymbolIndex` trait + inject into view/edit/analyze — Then

Define the trait. Implement three backends: daemon (warm), SQLite (cold), parse-on-
demand (no index). Wire into `view`, `edit`, `analyze` so they stop opening parsers
directly. This is the largest change — it touches the most command code.

### 6. Embedding warm-up in daemon for semantic search — Then

Background re-embedding on file change. Depends on the semantic retrieval
infrastructure from `docs/design/semantic-retrieval.md`. The daemon is the natural
home for keeping embeddings fresh.

## Resolved Decisions

- **Daemon is global, not per-project.** One daemon process manages multiple roots.
  Per-project daemons waste resources and complicate lifecycle management. The
  protocol already supports multi-root (`Add`/`Remove`/`List`).

- **Index queries go to SQLite, not through daemon.** The daemon maintains the index
  but doesn't mediate every read. SQLite handles concurrent readers natively. The
  daemon's value is in incremental writes, not in proxying reads.

- **Trait APIs, not protocol awareness.** Commands use `SymbolIndex` / `SemanticSearch`
  traits. Whether the implementation talks to a daemon socket, opens a SQLite file,
  or parses on demand is invisible to the caller. This means the daemon can be
  entirely absent (no Unix socket support, CI environment) and commands still work.

- **No full AST storage.** Symbol-level granularity is the sweet spot: cheap to
  maintain, covers most query patterns. Raw parsing is always available via `syntax`
  commands for the cases that need it.

- **Fire-and-forget, not fire-and-wait.** The CLI never blocks on daemon startup.
  If the daemon isn't ready yet, fall back to cold path. The user never waits for
  the daemon — they get progressively faster results as it warms up.
