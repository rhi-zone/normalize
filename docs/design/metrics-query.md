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

## Non-goals

- Not a BI tool. The queries are over code metrics, not arbitrary data.
- Not a replacement for `sqlite3 .normalize/index.sqlite`. Power users can always
  drop to raw SQL. This is about making common queries easy.
