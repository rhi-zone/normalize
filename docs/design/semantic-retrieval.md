# Semantic Retrieval — Design

## Framing

Normalize as an **acceleration structure** for code intelligence. The goal is not just
to answer queries correctly, but to make each query cheaper over time as the structure
accumulates.

Humans navigate large codebases fast because they have a mental model: they know where
things are, how they relate, what's alive versus dead. That mental model is retrieval —
pattern-matched lookup rather than search. The question is whether we can externalize
that model in a form that compounds.

The index already captures structural facts. This design adds a **semantic retrieval
layer** on top: vector embeddings over structurally-derived chunks, weighted by commit
activity and staleness, queryable by meaning rather than name.

This is essentially RAG infrastructure without the G. The value is in having better
chunks and better weights than naive document chunking. The retrieval result goes
directly to callers (agents, developers, tools) — normalize has no LLM calls.

## What Gets Embedded

Embedding sources, in rough priority order:

| Source | Granularity | Signal |
|--------|-------------|--------|
| Symbols + doc comments | Per symbol | Highest — colocated, author-written, versioned |
| Co-change clusters | Per cluster | Behavioral coupling, not just structural |
| Markdown docs (SUMMARY.md, CLAUDE.md, ADRs) | Per section | Intent and design rationale |
| Commit messages | Per commit | Change narrative |

What gets embedded is **configurable per repo** — different codebases have different
tribal knowledge distributions. Some live in commit messages, some in inline docs, some
in architecture decision records.

### Context Window Construction

Flat embeddings over isolated symbols lose too much signal. Each chunk is constructed
as a rich context window:

```
symbol name + signature
+ doc comment
+ parent module/crate path
+ callers (top N by frequency)
+ callees
+ co-change neighbors
```

The quality of the embedding is directly upstream of the quality of the index. Better
extraction → better context windows → better embeddings. The pipeline compounds.

## Weighting and Invalidation

Not all embedded content is equally trustworthy. Two signals downweight or invalidate:

**Commit activity** — things being touched are alive and relevant. Things untouched in
many commits are either stable bedrock or dead weight; co-change patterns distinguish
which. High-churn items get higher retrieval weight.

**Staleness** — documentation that hasn't been updated since the code it describes last
changed is suspect. Two staleness signals compound:

1. *Temporal staleness*: commits since last doc update (already tracked by stale-summary
   and stale-doc rules)
2. *Colocation staleness*: a comment in the function body is more trustworthy than a
   SUMMARY.md two directories up that hasn't moved in 50 commits. Proximity to the code
   matters.

Both signals come from git history — no new data source needed.

## Doc Comment Preprocessing

Before embedding, doc comments are preprocessed to extract clean prose: comment markers
are stripped via tree-sitter (query the doc comment node's text content field — not
heuristic string stripping), then line-wrap newlines are collapsed. This alone captures
most of the value.

Stretch goal: structured parsing of `Parameters:`, `Returns:`, `Panics:`, `Example:`
sections into typed fields for more discriminative embedding. Not needed for a first
implementation.

## Storage

Vectors stored in SQLite alongside the existing schema (new table, new schema version).
Populated during `structure rebuild`. Queried via ANN search.

**Implementation:** `fastembed` (Rust-native, ONNX-backed, no server, no API calls) +
`sqlite-vec` for vector storage and ANN search.

**Schema:**

```sql
CREATE TABLE embeddings (
    id           INTEGER PRIMARY KEY,
    source_type  TEXT NOT NULL,  -- 'symbol' | 'doc' | 'commit' | 'cluster'
    source_path  TEXT NOT NULL,
    source_id    INTEGER,        -- FK into symbols table where applicable
    model        TEXT NOT NULL,  -- embedding model used (for invalidation)
    last_commit  TEXT,           -- git commit hash when this was last embedded
    staleness    REAL NOT NULL DEFAULT 0.0,
    embedding    BLOB NOT NULL   -- f32 array, length = model dimensions
);
```

**Default model:** `nomic-embed-text-v1.5` (768 dims, mixed code+text, matryoshka
support — can query at 256/512 for speed). Configurable in `config.toml`; changing the
model requires a full rebuild (the `model` column triggers automatic invalidation).

**Fast option:** `all-MiniLM-L6-v2` (384 dims) for large codebases where query latency
matters more than precision. Code-specific models (jina, CodeBERT) are overkill — our
chunks are predominantly natural language with identifiers, not raw code.

## Rebuild Strategy

Follows the standard normalize pattern:

- **Full rebuild**: recompute all embeddings during `structure rebuild`
- **Incremental update**: re-embed only chunks whose source content has changed since
  last rebuild (git diff against stored HEAD)
- **Daemon incremental**: daemon watches for file changes and queues re-embedding in the
  background, so retrieval is fresh without a manual rebuild

## Surface

New command: `normalize structure search <query>` (name TBD). Returns ranked chunks with
source location, similarity score, and staleness metadata.

The retrieval result is structured data — `--json` works like every other command.
Agents consume this to orient themselves before acting; developers use it to find
conceptually related code without knowing exact names.

## What This Is Not

- Not a generative step — no LLM calls, no synthesis
- Not a replacement for the structural index — embeddings are a retrieval layer over it,
  not a substitute
- Not external RAG infrastructure — everything runs locally, everything is versioned in
  git, no network dependencies

## Resolved Decisions

- **One vector per chunk**, where chunk = context window centered on a symbol or doc
  section. They are the same thing.
- **Re-ranking at query time**, not baked into the vector. Staleness changes every
  commit; re-embedding on every commit would be too expensive. Store raw vector +
  staleness metadata; apply weighting as a post-ANN re-ranking pass.
- **Command:** `normalize structure search <query>` — lives under `structure` because it
  queries the structure index, consistent with `structure rebuild` / `structure status`.
- **Output:** structured JSON like every other command; `--json` / `--jq` work
  automatically.
