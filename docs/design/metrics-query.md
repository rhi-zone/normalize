# Metrics Query Primitive — Design (early)

## Framing

`analyze`, `rank`, and `trend` are the same operation: query metrics with different
reducers. They should unify into a single primitive. But the current implementations
hardcode equations, cutoffs, and windows. Users can't customize what "hot" means or
how moving averages are computed.

## Insight

The real primitive isn't "trend" or "rank" — it's **query over metrics**. The user
provides the query (metric, grouping, aggregation, filter, window), normalize
executes it over the index.

## What the primitive needs to express

- **Distribution**: what does the spread look like right now?
- **Trend**: how is a metric changing over time?
- **Correlation**: what moves together?
- **Outliers**: what's abnormal?
- **Aggregation**: roll up by group (file, directory, crate, language)

These are all: metric source × grouping × reducer × filter.

## Candidate backends

**SQL (SQLite)**: the index is already SQLite. Window functions, GROUP BY, CTEs
can express all of the above. But raw SQL is bad UX.

**Datalog**: already used for fact rules. Good for relational queries, awkward for
numeric aggregation and time series. Wrong paradigm for this.

## Open questions (parked)

- **Performance**: can SQLite handle windowed aggregation over 58k symbols × N
  commits efficiently?
- **LLM interface**: how well do LLMs write SQL? If the answer is "very well,"
  then SQL-as-backend with natural language as the query interface might be the
  right stack. The LLM generates the SQL, normalize executes it.
- **DSL vs flags vs SQL**: what's the right user-facing interface? A query DSL
  that compiles to SQL? Composable flags (`--group-by file --aggregate avg
  --window 10`)? Raw SQL with helpers? Or just let the LLM write it?
- **Data sources**: code metrics (from index), session metrics (from logs), rule
  violations (from rules engine) — all should be queryable through the same
  primitive. What schema unifies them?
- **analyze/rank/trend unification**: once the query primitive exists, these three
  commands become presets (canned queries with nice formatting). The primitive is
  the real command; the presets are aliases.

## Type safety (critical concern)

Library consumers need typed query results without runtime HashMap overhead.
Returning `Vec<HashMap<String, Value>>` for arbitrary queries is unacceptable
for hot paths.

### Options explored

1. **Presets only** — current 37 subcommands stay as typed structs. Safe but
   doesn't extend to custom queries.

2. **Columnar storage** — columns stored contiguously with a schema header.
   No per-row allocation but no compile-time type safety either.

3. **Registered views** — pair SQL with a Rust struct via derive macro. The view
   is the typed contract. Compile-time checked against the schema. Library
   consumers define views; the macro validates them.

4. **sqlx** — the obvious answer. `query_as!` gives compile-time SQL checking
   against the real schema. Zero runtime overhead. The industry standard.
   **Problem**: sqlx doesn't support libsql as a backend, and we use libsql
   for the entire index (facts, semantic, rules, daemon). Switching to vanilla
   SQLite would lose libsql's built-in vector search and replication. Running
   both is messy (two connection pools to the same DB).

### Resolution: libsql `from_row` with serde

libsql already has `libsql::de::from_row<T: Deserialize>(&Row) -> Result<T>`.
This gives typed deserialization without switching to sqlx:

```rust
#[derive(Deserialize)]
struct FileComplexity {
    file: String,
    avg_complexity: f64,
    count: i64,
}

let mut rows = conn.query("SELECT file, AVG(complexity) as avg_complexity, 
    COUNT(*) as count FROM symbols GROUP BY file", ()).await?;
while let Some(row) = rows.next().await? {
    let result: FileComplexity = libsql::de::from_row(&row)?;
}
```

No HashMap, no runtime type guessing, serde handles the column→field mapping.
The struct is the schema contract. Library consumers define their own result
structs with `#[derive(Deserialize)]` and pass them to `from_row`.

**What we get**: typed deserialization at runtime, zero per-row allocation
overhead, stays on libsql (no migration needed), works for both presets and
custom queries.

**What we don't get**: compile-time SQL validation (sqlx's killer feature).
The SQL string is still unchecked — a typo in a column name is a runtime error,
not a compile error. Acceptable tradeoff: we can add integration tests for
preset queries, and custom queries are the user's responsibility.

This unblocks the design. The query primitive can proceed on libsql.

## Non-goals

- Not a BI tool. The queries are over code metrics, not arbitrary data.
- Not a replacement for `sqlite3 .normalize/index.sqlite`. Power users can always
  drop to raw SQL. This is about making common queries easy.
