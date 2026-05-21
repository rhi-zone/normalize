# Graph Substrate: Design Sketch

A persistent, addressable, queryable medium for structured thought adjacent to a codebase — implementing the primitive proposed in [`../introspection/graph-substrate-thesis.md`](../introspection/graph-substrate-thesis.md).

**Status (2026-05-21):** v0 shipped as `normalize-knowledge-graph` crate, CLI noun `kg`. Decisions below marked **[LOCKED]** were settled by the approved plan and are implemented. Open questions that remain are unchanged.

## Motivation

Two concrete gaps motivate building this, but the substrate is general — neither use case should drive the primitives.

- **Codebases have no wiki.** Code has comments, READMEs, `docs/`, rustdoc — none compose into a queryable, cross-linkable medium of understanding adjacent to (and addressable from) the code itself. GitHub wikis exist but are second-class and dead. The understanding of a codebase — invariants, history, why a thing is the way it is, what's been tried and rejected — has no home.
- **Large design tasks can't be one-shot, even by frontier models.** A complex typechecker is weeks of partial designs, contradictions, alternatives, dead-ends, refinements. Conversation can't host this (linear, transient). Code can't host this (too concrete, no room for what was rejected). `docs/` can't host this (not iterative, not editable mid-design). The medium for *iteratively constructing* a large design across sessions doesn't exist.

These are evidence the gap is real, not the spec. Other plausible uses: decision logs, investigation registries, knowledge bases, daily journals, task trackers, research notebooks. The substrate doesn't bake any of them in. Generality at the primitive layer is the point; specificity lives in conventions on top.

## Goals (v0)

The minimum viable substrate must support:

1. **Addressable units** — every unit of state has a stable id that can be referenced from a prompt, a link, or a query.
2. **Free-form body + structured metadata** — body is opaque to the substrate (markdown by convention); metadata is structured (so machines can query).
3. **Typed cross-references** — units link to other units via edges with kinds; the link graph is queryable.
4. **Filesystem-backed and git-friendly** — units are files; the graph is diffable, committable, branchable. No opaque database that breaks `git diff`.
5. **Queryable** — "give me all units where `tag=hypothesis` and there's an edge of kind `evidence-for` to unit X" should be a single command, not a script.
6. **Cheap to read for an LLM** — fetching a unit + its immediate neighbors should be one command with predictable output.
7. **Cheap to update from any process** — appending, changing metadata, creating a unit, adding an edge — each should be one command.

## Non-goals (v0)

- A new editor or UI. The substrate is read/written via CLI and a text editor.
- Real-time multi-writer coordination. Concurrent writes are handled with file locking + last-writer-wins; richer concurrency is deferred.
- Versioning beyond what git provides. Each unit's history is its git history.
- A query language richer than predicate matching + neighbor traversal. SPARQL-grade graph queries are out of scope for v0.
- Cross-repo graphs. Each repo has its own `.normalize/substrate/`. Cross-substrate references are an open question (see below); v0 assumes single substrate.
- A general-purpose knowledge graph (inference, semantic relations, ontology). This is a workflow substrate, not an ontology engine.

## Architectural fit

Three crates already exist that this substrate composes from rather than competes with:

- **`normalize-context`** is a filesystem-walk-only store of markdown-with-frontmatter, parsed on every resolution call. No index, no cache, no staleness. The right pattern for human-edited content of modest scale.
- **`normalize-facts`** runs a content-addressed cache keyed on `(blake3(bytes), extractor_version, …)`. The normalize pattern for "derived data from source of truth," self-invalidating: if the source's hash matches a cache entry, reuse; otherwise re-derive. Mtime is not used.
- **`normalize-graph`** is pure graph algorithms (Tarjan SCC, bridge-finding, dependents BFS) over abstract adjacency. Storage-agnostic.

The substrate is therefore not a new storage paradigm. It is **`normalize-context` with addressable IDs and typed edges, indexed via the same content-addressed cache pattern as `normalize-facts`, fed into `normalize-graph` for queries**. The composition is the design.

Per the project's "own crate when standalone-useful or multi-consumer" rule, the substrate is a new crate (working name `normalize-substrate`) that depends on `normalize-context` (or generalizes it) and exposes its own `#[cli(...)]` service. The main `normalize` binary mounts it like any other feature crate.

What this does NOT do: invent a new event log, dual-write to markdown and SQLite as parallel sources of truth, or treat git as a transactional store. Git is the history because the files are git-tracked; that's incidental, not architectural.

## Prior art and why not just use it

- **Obsidian / Logseq / Foam.** Markdown-with-frontmatter graphs with wikilinks. Excellent for humans. Wrong shape: GUI-first, no CLI-first query language, no programmatic write API that subagents can invoke, no first-class anchors into code.
- **Datasette / sqlite-utils.** Strong CLI surface on SQLite. Wrong source of truth: structured rows, not markdown bodies. Humans don't edit it ergonomically; subagents would have to learn schemas instead of writing prose.
- **`git-bug` / `git-appraise`.** Git-native append-only issue/review stores. Closest in spirit. Wrong scope: hard-coded to issues/reviews, not extensible to arbitrary unit kinds.
- **Plain `docs/` + `grep`.** Today's baseline. Fails the "queryable by metadata" goal and the "read unit + neighbors in one call" goal.
- **`normalize-context` as-is.** Closest existing thing. Missing: stable IDs, typed edges, neighbor traversal, write API, code anchors. The substrate is what `normalize-context` becomes when those are added.

The novel piece is not the file format or the graph algorithms — both are off-the-shelf. The novel piece is **continuous bidirectional sync between any process (human, LLM session, subagent, hook) and a persistent shared workspace** that lives next to the codebase, with anchors that connect the workspace to the code.

## Primitives

The strong composable core is three primitives. Everything else is convention on top.

### 1. Unit

The addressable atom of content.

- **ID:** user-chosen slug (ergonomic) with content-hash fallback (machine-generatable).
- **Body:** opaque to the substrate. Any content-type — markdown by convention, but could be image, PDF, JSON, binary. The `content-type` is just a metadata key.
- **Metadata:** arbitrary key-value. No required keys. Conventions populate `status`, `type`, `kind`, etc. as they see fit. The substrate does not privilege any key beyond what's needed for storage/query bookkeeping.

### 2. Edge

A typed, directed relationship between two units.

- **Shape:** `(from, to, kind, optional metadata)`.
- **Kind** is a freeform string. The substrate doesn't know what `supersedes` means; conventions do.
- Edge kind is primitive (not encoded as metadata on a unit) because the asymmetry is real: a `supersedes` edge means something different from an `evidence-for` edge in a way no metadata convention reduces. Edges are how the graph gets its shape.

### 3. Query

Predicate over units, edges, and metadata.

- "Reach all units where `metadata.X = Y` and there's an edge of kind `K` to unit `Z`."
- Standard traversal patterns (children-of, descendants, predecessors, link-graph) are convenience over this primitive.

No `type` at the primitive layer. No `status`, no `parents`, no `intent`. Those are all conventions. The substrate cares about identity, relationships, and queryability — not about how any particular consumer organizes meaning.

## Standard library (conventions on top)

Documented but not enforced. Conventions can live in wrapper scripts or in `normalize` subcommands; the substrate doesn't care.

### Anchors

How a unit connects to anything outside the substrate. Recommended schema:

```yaml
metadata:
  anchors:
    - symbol: Frobnicator
      crate: normalize-foo
      path: crates/normalize-foo/src/frobnicator.rs
      commit: abc123
    - url: https://example.com/spec
```

An anchor is a **bag of identifying facts**. Queries match on any subset (`anchors[].symbol = X`, `anchors[].crate = Y`, `anchors[].path = Z`). The substrate doesn't canonicalize — it stores the facts and matches on any of them.

Redundancy is the feature: multiple facts mean multiple query paths, self-validating links, and graceful degradation when one fact goes stale. If a symbol is renamed, queries by old symbol still hit historical anchors; queries by current symbol require the anchor to have been updated by something that knows how (a hook calling `normalize structure`, a maintenance command). Resolution lives outside the primitive.

Anchors aren't strictly primitive — they're structured metadata — but they're the most important convention because they're how the substrate stops being a closed notebook and becomes part of a world.

### Type-as-tag

Common to tag units with a `type` or `kind` metadata key — `wiki-page`, `design-fragment`, `decision`, `note`. Useful for queries (`find all decisions about typechecker`). At the substrate level it's just metadata; conventions assign meaning.

### Status

Lifecycle: `open`, `in-progress`, `done`, `discarded`. Useful for task-shaped conventions; meaningless for wiki-shaped ones. Always a metadata key, never privileged.

### Standard edge kinds

A non-exhaustive vocabulary that conventions converge on: `parent-of`, `supersedes`, `derived-from`, `alternative-to`, `evidence-for`, `red-teamed-by`, `relates-to`. Conventions extend as needed.

### Rollups and current-view

A query pattern, not a primitive. "Show me the leaf of the `supersedes` chain reachable from root X" or "show me the most-recently-updated unit tagged `current-view` under X." Some conventions maintain an explicit rollup unit; others derive it on demand.

## Conventions illustrated

Two convention sketches showing how the primitives compose. Neither is part of the substrate; both could be built atop a stable v0 substrate. They are illustrations of the kind of thing conventions are, not a fixed list.

### Codebase wiki

- Units tagged `kind: wiki-page` with anchors pointing at code (symbols, paths, crates).
- Edges of kind `references`, `relates-to`.
- Surfaced inline by `normalize view <file>` listing related wiki pages by anchor match.
- Anchors maintained by a hook on `normalize structure rebuild` that detects renames and updates anchor facts.

### Design workspace

- Units tagged variously: `fragment`, `alternative`, `decision`, `open-question`.
- Edges: `alternative-to`, `supersedes`, `decided-by`, `blocked-on`.
- "Current best understanding" is a query: latest unit with `tag: current-view` for a given root, or the leaf of a `supersedes` chain.
- A fresh session reads the workspace and inherits the accumulated design state at full fidelity.

Other plausible conventions (decision logs, investigation registries, daily journals, task trackers, research notebooks) build on the same primitives. The substrate doesn't care which exist.

## Storage layout **[LOCKED: v1]**

```
.normalize/kg/
└── <id>.md                       # one file per unit — source of truth (ID grammar: [a-z0-9][a-z0-9-]*)
```

Each unit file has YAML frontmatter with an optional `links` field holding outgoing edges:

```yaml
---
tag: wiki-page
anchors:
  symbol: Frobnicator
links:
  - kind: references
    to: other-unit
  - kind: derived-from
    to: another-unit
    metadata:
      note: background context
---
Body text here.
```

**Locked decisions:** storage root is `.normalize/kg/` (not `.normalize/substrate/`); no namespace directories in v0; ID grammar is `[a-z0-9][a-z0-9-]*`; outgoing edges stored in each source unit's `links` frontmatter array as `{kind, to, metadata?}`.

Source of truth: the filesystem. There is no parallel write target. Everything else is derived.

**Indexing follows the `normalize-facts` content-addressed pattern.** Per-unit cache rows in `~/.config/normalize/ca-cache.sqlite` (the existing CA cache, extended with a new namespace) keyed on `(blake3(unit_bytes), substrate_version)`. On any read:

1. Hash each unit file in the working tree.
2. Look up `(hash, version)` in the CA cache. Hit → reuse parsed metadata + link set. Miss → parse, store, return.
3. Query the cache for the requested view.

Consequences:
- **No staleness ambiguity.** If a hash matches a row, the row is correct by construction. If no row matches, parse cost is paid exactly once per content version.
- **No mtime, no watch loops, no "rebuild on stale."** The cache is never wrong; at worst it's empty for new content.
- **No gitignored generated SQLite inside the repo.** The cache lives in the user's config dir (per existing convention) and is fully reconstructible from the tree.
- **Manual edits and CLI writes are indistinguishable.** Both produce a file change; the next read hashes and indexes it. Any process (human, subagent, hook) uses the same write semantics — write the file.

Edges are stored in each source unit's frontmatter `links` array. Adding or removing an edge rewrites the source unit atomically (write-to-temp, rename). Outgoing edges are local to each unit's file; incoming edges are found by scanning all units for `link.to == id`.

`normalize-graph` consumes the adjacency built from walking all units' links. Traversal, reachability, structural queries reuse its existing algorithms; the substrate provides adjacency, not new graph code.

Concurrency follows file granularity: concurrent writes to *different* units don't collide; concurrent writes to the *same* unit use file locking with last-writer-wins. Append-style operations are read-modify-write under that lock. v0 doesn't attempt finer-grained merges.

**Migration:** if `.normalize/kg/edges.jsonl` is present (legacy v0 log format), the first kg command automatically projects the current edge state, writes each edge into the corresponding source unit's frontmatter, and renames the log to `edges.jsonl.migrated-v0`. One-shot; idempotent after rename.

## CLI surface **[LOCKED: v0]**

Flat verbs under `normalize kg`. One nesting level, no sub-nouns. Output respects `--pretty/--compact/--json/--jsonl/--jq` automatically via server-less.

```bash
normalize kg create   [id] [--metadata key=val ...]        # body via stdin
normalize kg get      <id>
normalize kg set      <id> [--metadata key=val ...]
normalize kg append   <id>                                  # body via stdin
normalize kg delete   <id>

normalize kg link     --from A --to B --kind K [--metadata key=val ...]
normalize kg unlink   --from A --to B --kind K
normalize kg edges    [--from A] [--to B] [--kind K]

normalize kg query    [--match key=value ...] [--edge-kind K] [--connected-to ID]
normalize kg neighbors <id> [--depth N] [--edge-kind K]
normalize kg show     <id> [--depth N]                      # unit + neighbors
```

No convention layer in v0. `normalize wiki`, `normalize design`, etc. are deferred — primitives are the surface.

## Decisions log

- **Edges-in-log vs edges-in-frontmatter.** **[DECIDED: per-unit frontmatter]** v0 shipped an append-only `edges.jsonl` log with tombstone projections. Flipped to per-unit frontmatter (`links` array) because the shared log is git-unfriendly: every concurrent branch that appends an edge produces a merge conflict at EOF. Per-unit ownership matches the Obsidian/Logseq pattern — edge changes show up in the source unit's diff, which is where they belong. Whole-unit rewrites are microseconds; we don't have a write-rate problem. v0 data auto-migrates on first run.

## Open questions
- **Anchor resolution lifecycle.** Who updates anchors when code changes? A hook on `normalize structure rebuild`, an explicit `normalize kg anchors refresh`, both? What happens to stale anchors — surfaced, archived, ignored? **[OPEN]**
- **Cross-substrate references.** Anchor schema admits URLs and could admit substrate-relative paths; whether substrates can reference each other as first-class is open. **[OPEN]**
- **Body encodings.** Markdown is convention. If a unit's body is binary, what does `kg get` print? Conventions around `content-type` need design. **[OPEN, deferred from v0]**
- **Where convention sugar lives.** `normalize wiki`, `normalize design` etc. are explicitly **out of scope for v0** — no convention commands ship. Primitives are the surface. **[LOCKED: no convention sugar in v0]**
- **Continuous-sync ergonomics.** The substrate is durable, but writing into it still requires *something* (a command, a hook, an edit). What patterns make "the live mental model is in the substrate" actually true in practice, vs aspirational? **[OPEN]**
- **History granularity.** Each unit's history is its git history at file granularity. Sub-unit changes (one metadata field changing) are visible only as whole-file diffs. Sufficient for v0; richer change-tracking is deferred. **[OPEN]**
- **CA cache.** v0 does NOT use a CA cache. Walk-the-tree on every read, per `normalize-context` pattern. Add the cache when read latency hurts. **[LOCKED: no cache in v0]**

## Smallest viable prototype **[SHIPPED]**

Shipped as `normalize-knowledge-graph` crate, mounted as `normalize kg`:

- `normalize kg create / get / set / append / delete`
- `normalize kg link / unlink / edges`
- `normalize kg query / neighbors / show`
- Markdown-with-frontmatter storage; outgoing edges in per-unit `links` frontmatter. No CA cache (walk-the-tree). Auto-migration from legacy `edges.jsonl` log.
- No convention layer — primitives are the surface.

Friction discovered there shapes the convention layer and any harness integration. The primitives don't change.
