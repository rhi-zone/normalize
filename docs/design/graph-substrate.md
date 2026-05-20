# Graph Substrate: Design Sketch

A sketch of the primitive proposed in [`../graph-substrate-thesis.md`](../graph-substrate-thesis.md).

**Caveat:** This document was drafted in the same Claude Code session as the thesis it implements. It is informed by that session's reasoning but not independently validated. Treat it as a starting point for the actual design work, not a finished design. Specifically: the schema is a first guess, the API surface is sketchy, the storage decision is unresolved, and integration with the Claude Code harness is described at the wrapper level without verifying which hooks are actually available. Pre-implementation, every section here deserves a fresh look.

## Goals (v0)

The minimum viable substrate must support:

1. **Addressable nodes** — every unit of state has a stable id that can be referenced from a prompt, a link, or a query.
2. **Free-form body + structured metadata** — node content is markdown (so humans can write/edit); metadata is structured (so machines can query).
3. **Explicit cross-references** — nodes link to other nodes; the link graph is queryable.
4. **Filesystem-backed and git-friendly** — nodes are files; the graph is diffable, committable, branchable. No opaque database that breaks `git diff`.
5. **Queryable** — "give me all pending children of node X with type=hypothesis" should be a single command, not a script.
6. **Cheap to read for an LLM** — fetching a node + its immediate context (parents, children, linked nodes) should be one command with predictable output.
7. **Cheap to update from a subagent** — appending a result to a node, changing its status, creating a child, should each be one command.

## Non-goals (v0)

- A new editor or UI. The substrate is read/written via CLI and a text editor.
- Real-time multi-writer coordination. Concurrent writes are handled with simple file locking or last-writer-wins; richer concurrency is deferred.
- Versioning beyond what git provides. Each node's history is its git history.
- A query language richer than `--match` filters + neighbor traversal. SPARQL-grade graph queries are out of scope for v0.
- Cross-repo graphs. Each repo has its own `.normalize/graph/`. Cross-repo linking is deferred.
- A general-purpose knowledge graph (with inference, semantic relations, etc.). This is a workflow substrate, not an ontology engine.

## Data model

### Node

Every node is a markdown file at `.normalize/graph/<id>.md`. Frontmatter holds metadata; body is the content.

```markdown
---
id: hypothesis-correction-tax
type: hypothesis
status: alive
intent: |
  Encoded constraints are violated because the encoding is leaky.
  Specifically: rules that name failure modes work; rules requiring
  the user's generator do not.
parents: [investigation-2026-05-20-whats-wrong]
links:
  - kind: derived-from
    to: investigation-2026-05-20-whats-wrong/recon
  - kind: relates-to
    to: hypothesis-implicit-constraints
  - kind: red-teamed-by
    to: red-team-correction-tax
created: 2026-05-20T16:14:00Z
updated: 2026-05-20T17:42:00Z
---

# Correction Tax (alive, narrowed)

Five distinct correction classes recur across sessions separated by
days to weeks, after CLAUDE.md encoding. The clearest mechanism is
the reactive-bandaid loop: rule added in response to violation → rule
itself violates the no-bandaid principle → rule deleted → behavior
recurs.

## Evidence For
...

## Caveats
...
```

### Fields (frontmatter)

| Field      | Type             | Required | Purpose                                                            |
|------------|------------------|----------|--------------------------------------------------------------------|
| `id`       | slug             | yes      | Stable identifier. Must match filename without `.md`.              |
| `type`     | string           | yes      | Node-kind, e.g. `hypothesis`, `task`, `decision`, `evidence`.      |
| `status`   | string           | no       | Lifecycle state, e.g. `open`, `in-progress`, `done`, `discarded`.  |
| `intent`   | string (multiline) | no     | Brief statement of what this node is for. Read by subagents.       |
| `parents`  | list[id]         | no       | Hierarchical parent(s). Multiple allowed (graph, not tree).        |
| `links`    | list[{kind, to}] | no       | Typed edges to other nodes.                                        |
| `created`  | ISO 8601         | yes      | Set on creation.                                                   |
| `updated`  | ISO 8601         | yes      | Set on every update.                                               |
| ...        | any              | no       | Convention-specific extension fields (e.g. `evidence_session_ids`).|

Conventions on top (workqueue, investigation, decision-log, design space) add their own conventional fields without changing the substrate.

### Edges

Two edge representations:

- **Frontmatter `links`** — explicit typed edges. Used for structural relationships (parent, derived-from, supersedes, relates-to, red-teamed-by, evidence-for). Queryable directly.
- **Wikilinks in body** — `[[other-node-id]]` in markdown. Parsed lazily by the query layer to surface "what links to X." Less structural, more associative.

Both are first-class. Frontmatter is for relationships you'd query; wikilinks are for relationships that emerge from writing.

### Storage layout

```
.normalize/graph/
├── <id>.md                       # one file per node
├── <namespace>/<id>.md           # optional: namespace by type or convention
└── .index/                       # generated SQLite cache (gitignored)
    └── nodes.sqlite              # frontmatter + link index for fast queries
```

Filesystem is the source of truth. SQLite is a derived cache, rebuilt on-demand by `normalize graph index` or automatically when stale. This preserves git-friendliness while keeping queries fast at scale.

## CLI surface

All commands under `normalize graph`. Output respects existing `--pretty/--compact/--json/--jsonl/--jq` conventions from elsewhere in normalize.

### Read

```bash
# Fetch a node (frontmatter + body)
normalize graph show <id>

# Fetch a node plus immediate neighbors (parents, children, linked)
normalize graph context <id> [--depth 1]

# Query nodes by metadata
normalize graph query --match status=open --match type=task
normalize graph query --match 'parent=hypothesis-correction-tax'
normalize graph query --any 'status=open' 'status=in-progress'

# Traverse
normalize graph neighbors <id> [--kind links.derived-from]
normalize graph subtree <id>          # all descendants
normalize graph ancestors <id>        # all ancestors

# List
normalize graph list [--type X] [--status Y]
```

### Write

```bash
# Create a node (id auto-generated if not provided; body via stdin or -m)
normalize graph create --type task --parent <parent-id> --intent "..." -m "body..."
normalize graph create --type hypothesis --id custom-id < body.md

# Update metadata
normalize graph set <id> --status done
normalize graph set <id> --link kind=derived-from to=<other-id>
normalize graph unlink <id> --to <other-id> --kind <kind>

# Append to body (subagent result, for instance)
normalize graph append <id> < new-content.md

# Discard (set status=discarded; does not delete the file)
normalize graph discard <id>

# Permanently delete (probably never used directly)
normalize graph delete <id>
```

### Index / maintenance

```bash
normalize graph index               # rebuild SQLite cache
normalize graph validate            # check for broken links, schema violations
normalize graph migrate <from> <to> # rename id, update all references
```

## Integration with the Claude Code harness

Normalize itself does not integrate with Claude Code. Integration lives in a shell wrapper (call it `claude-graph` or similar) that calls `claude` with hooks/prompts arranged to use the substrate. This keeps normalize agnostic of the harness.

Sketch of what the wrapper does:

### On session start

Before invoking `claude`, the wrapper:

1. Inspects the working directory and recent git activity.
2. Runs `normalize graph query` to find active nodes (status=in-progress, recently touched).
3. Injects them as initial context via `claude`'s `--append-system-prompt` or via a `SessionStart` hook that calls `normalize graph context <root>`.

Result: the session opens with the relevant subgraph already loaded. No "what were we working on" reconstruction.

### On subagent dispatch (PostToolUse hook on Agent / Task tool)

When the model calls Agent / Task, the wrapper:

1. Parses the prompt for a `--node <id>` argument (convention: subagent prompts include the node they're working on).
2. Sets the node's status to `in-progress`.
3. Lets the subagent run.
4. On completion, takes the subagent's return value and `normalize graph append <id> --section "Subagent result"`s it to the node.
5. Sets status to `done` or whatever the subagent's last line indicates.

Result: subagent work auto-persists to the substrate without the orchestrator having to remember to write it.

### On session end (Stop hook or wrapper postlude)

The wrapper:

1. Asks the model to summarize unwritten state into a node (or several).
2. Persists that summary.
3. Marks any in-progress nodes that the session closed without resolving.

Result: nothing important leaves the session without being captured.

### Slash command for explicit ops

A `/graph` skill in `~/.claude/commands/` exposes the common ops (create, query, link, append) as inline commands the model can use mid-conversation when hooks miss something.

## How subagents use the substrate

The dispatch contract: every subagent prompt names the node it's working on.

```
You are working on node `<id>`. Your task is the `intent` field of that node.
Read its context via `normalize graph context <id> --depth 2`. Return your
result. The wrapper will append it to the node and update its status.
```

The subagent reads:
- The node itself (intent, prior content)
- Parents (broader context, root intent)
- Children (sub-work already done)
- Linked nodes (evidence, derived-from chain, related work)

It does NOT need a hand-written prompt that re-states main session's context. The substrate IS the context channel.

Result: the prompt-as-sole-bridge weakness identified in the thesis is fixed at the dispatch interface. Subagents read intent at full fidelity from the substrate.

## Migration path

The substrate stands up alongside existing conventions; nothing is deleted in v0.

1. **First convention: investigation registries.** The investigation we just ran (`docs/introspection/investigations/2026-05-20-whats-wrong/`) is already almost a graph — directory of hypothesis files with implicit cross-references. Convert to substrate as the proof-of-concept: each hypothesis becomes a node, evidence becomes child nodes, red-team results become linked nodes, synthesis is the root.
2. **Second convention: per-project work queues.** Take one project's TODO.md, convert each item to a task node. See if the day-to-day experience improves.
3. **Third convention: decision logs.** When a non-trivial decision happens, capture as a node with alternatives-considered linked.
4. **Fourth convention: domain knowledge bases.** Type theory / Lua semantics / MLstruct notes as a linked graph for crescent's typechecker work.

At each step, the substrate schema may need extension. Don't try to design v0 with all four use cases in mind — design for the first, observe friction, extend.

## Open implementation questions

- **`.normalize/graph/` vs other location.** Could live alongside `.normalize/context/` (same machinery, expanded), in a separate subdirectory, or even in `docs/graph/` for visibility. Trade-off: code-adjacent vs docs-adjacent.
- **Wikilink resolution.** What if `[[some-id]]` references a node that doesn't exist yet? Create stub? Warn? Ignore? Probably warn-and-allow, with `normalize graph validate` surfacing broken links.
- **Concurrency.** Two subagents writing to the same node concurrently. v0: file locking, last-writer-wins with a warning. v1: maybe append-mode for some node types (logs), replace-mode for others (status).
- **GC and archive.** Discarded nodes stay in the filesystem. At some point, do we archive them? Probably not until friction emerges.
- **Cross-project linking.** A node in crescent links to a node in normalize. Out of scope for v0 (each repo has its own graph), but the id schema should be designed to allow it later (URI-shaped, not just slug-shaped).
- **Body conventions.** Should the body have suggested sections (Claim, Evidence, Caveats) per node type? Or stay free-form? Probably free-form for v0; conventions document recommended sections per type.
- **Search.** Full-text search over node bodies. Probably defer to `normalize grep` extended to handle the graph, or `ripgrep .normalize/graph/`. Don't reinvent.

## What this design does NOT yet specify

- Whether SQLite is required or whether a pure-filesystem v0 is viable (probably depends on graph size).
- The exact shape of the SessionStart hook's injection (raw text? rendered prompt? structured JSON the model is expected to parse?).
- How the slash command interacts with subagent dispatch (does `/graph create` from within a subagent persist the same way?).
- The wrapper's exact name, install location, and configuration mechanism.
- Migration tooling for existing TODO.md / investigation directories.
- Whether the substrate needs an explicit "root intent" node per project, or if intent is inferred from the highest unparented node.
- Telemetry: does the substrate track how often nodes are read, by which sessions, to inform later quality-of-context measurement?

These should be answered in v0 implementation, not in the design doc.

## Smallest viable prototype

To validate the thesis, the minimum working version is:

- `normalize graph create / show / set / query` (4 commands).
- Markdown-with-frontmatter storage, no SQLite cache initially.
- One convention (investigation registries) migrated.
- One shell wrapper that invokes `claude` with SessionStart loading the active investigation node.

That's enough to dogfood. Friction discovered there shapes the rest of the design.
