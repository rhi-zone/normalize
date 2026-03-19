# Agent Query Interface

Design decisions for making normalize useful to AI agents exploring codebases.

## The Problem

Agents exploring a codebase need to answer abstract questions:
- "What does this codebase do?"
- "How does this codebase call external APIs?"
- "Where is the agentic loop?"
- "What caching library does it use?"
- "Why does it use rayon and not tokio?"

These questions are not navigational (find symbol X) — they are comprehension questions.
The answer is emergent from patterns across the whole codebase.

## Key Insight: Turn Count + Token Cost

The bottleneck is not index build time or query latency. It is:
1. **Round trips** — each turn an agent takes is expensive (latency, context, cost)
2. **Tokens per turn** — longer queries consume more of the agent's compute budget per token

The goal is: fewer turns per question, fewer tokens per turn.

## What Agents Actually Need

Abstract questions require *relational* information, not isolated facts.
Not "this file imports rayon" but "rayon is imported in 8 files, all in the
processing layer, all via par_iter(), never for I/O."

This is a query problem, not a summarization problem. The right primitive is
arbitrary relational queries over extracted structural facts.

## Solution: Expose the SQLite Index Directly

The structural index is already SQLite. The schema is:
- `files` — indexed paths with mtime and line count
- `symbols` — definitions with kind, visibility, parent, line range
- `symbol_attributes` — decorator/annotation facts
- `symbol_implements` — trait/interface implementation edges
- `calls` — caller → callee edges with file and line
- `imports` — import edges with optional resolution to local file
- `type_methods` — method membership
- `type_refs` — type usage edges

`normalize structure query <sql>` exposes arbitrary SQL queries against this index.
An agent can answer any structural question in one turn by writing the appropriate query.

No LLM is involved in normalize. The tool stays deterministic and testable.

## Why Not Pre-Built Commands Per Question Type?

Special-casing every question type is not viable — the space of questions is unbounded.
The right abstraction is a general query interface, not an enumerated set of views.

## SQL Views as Token Compression

Views are pre-defined query fragments that reduce the tokens an agent must generate
per query. Less generated SQL = less compute spent per token on reconstruction.

LLMs have finite compute per token. For domain-specific concepts not common in
training data, deriving the query from raw tables requires active reasoning.
Views offload that reasoning to precomputed schema structure.

**Views worth defining**: domain-specific code concepts that require non-trivial joins
and are unlikely to be precomputed in model weights:
- `entry_points` — public symbols with no callers
- `external_deps` — imports where `resolved_file IS NULL`
- `external_surface` — public symbols called from external_deps

**Views not worth defining**: concepts trivially expressed in SQL that models handle
in weights — `WHERE visibility = 'public'`, simple counts, etc.

## Measuring Exploration Cost: Subagent Session Analysis

To validate whether `structure query` (and views) actually reduce turn count, we need
to measure how agents explore today. Claude Code stores subagent transcripts at
`~/.claude/projects/<project>/<session>/subagents/agent-<id>.jsonl` — same JSONL
format as main sessions. These contain every tool call, token count, and search
pattern an Explore or general-purpose subagent used.

### Data Model

A subagent is a session with extra metadata:
- `parent_id` — the session that spawned it
- `agent_id` — the subagent's own ID (e.g. `agent-a5c5ccc9c2b61e757`)
- `subagent_type` — `Explore`, `general-purpose`, `Plan`, etc.

### CLI Surface

**`--mode interactive|subagent|all`** filter on `list`, `stats`, `messages`, `analyze`.
Default: `interactive` (current behavior). Flat listing with a `parent` column when
subagents are included. Comma-delimited to combine modes.

**`sessions show <session> subagents`** — summary table of a session's subagents
(type, turns, tokens, duration, tool breakdown).

**`sessions show <agent-id>`** — drilldown into a specific subagent as a full session.

**Inline display**: when showing a parent session, subagent tool calls display the
agent ID so the user can drill down directly.
