# Seam evaluation: metric core + git-history compute (A2 vs A1)

Read-only investigation. Question: do the metric and git-history algorithms currently
housed in the main `normalize` crate constitute genuine standalone crates (pass the
crate-existence bar on their own merits), or would extraction be a forced cut that
manufactures crates to serve a verb taxonomy?

The bar (CLAUDE.md): a crate exists only if **(a)** it has multiple actual workspace
dependents, or **(b)** it is clearly useful standalone (publishable, people would use it
without normalize — cf. `normalize-graph`, `normalize-code-similarity`). "Could
theoretically be reused someday" does not count. CLI aesthetics never drive structure.

---

## Candidate 1 — Metric core — **FAILS the bar (as a new crate)**

### Where the compute actually lives
- `crates/normalize/src/analyze/complexity.rs` (587 L) — `ComplexityAnalyzer`, `FunctionComplexity`,
  `RiskLevel`. Compute path: `analyze()` → `analyze_with_trait()` → walks `tags.scm`, then calls
  **`normalize_facts::extract::compute_complexity`** (`crates/normalize-facts/src/extract.rs:622`)
  per function. The cyclomatic core is *already in `normalize-facts`*; this file re-walks tags and wraps.
- `crates/normalize/src/analyze/function_length.rs` (380 L) — pure tree-sitter span counting via
  `normalize_languages::{Language, support_for_path}`.
- `crates/normalize/src/analyze/test_gaps.rs` (207 L) — needs the **call-graph index**.
- Per-command metrics in `crates/normalize/src/commands/analyze/`: `ceremony.rs`, `density.rs`
  (gzip-compression density via `flate2`), `size.rs`, `files.rs`, `length.rs`, `complexity.rs`,
  `imports.rs`, `surface.rs`, `test_ratio.rs`.

### Dependency reality
Two disjoint groups — **not one domain**:

| metric | depends on |
|---|---|
| complexity, length, ceremony, density, size, files | `normalize_languages` (Language trait, `GrammarLoader`, `support_for_path`), tree-sitter, source text, `crate::parsers` (thin loader wrapper). **No SQLite index.** |
| surface, imports, test_gaps, test_ratio | `crate::index::FileIndex` (`crates/normalize/src/index.rs`) — the **SQLite index**. Entangled with main-crate state. |

The AST group is generic substrate; the index group drags the main-crate index and cannot move
without extracting the index itself (a separate, larger question — not in scope here).

### Verdict reasoning
- **Not a coherent single domain.** It is a bag of per-command algorithms (gzip density, ceremony
  ratio, byte size, file count, span length, cyclomatic) whose only commonality is "iterate over
  code." That is not a shared core; it is the definition of CLI-command grouping.
- **The one genuinely-domain-logic, standalone-useful piece already has a home.** "Compute
  cyclomatic complexity / per-function spans over a tree-sitter tree" is exactly extraction-over-AST,
  which is `normalize-facts`' job — and `compute_complexity` is *already there*. The right move for
  the per-function complexity/length walking is **fold into `normalize-facts`**, not spawn a crate.
- A new `normalize-metrics`-style AST crate would: (i) **collide** with the existing
  `crates/normalize-metrics` (the ratchet/budget `Metric` trait + `Aggregate` — a different domain:
  snapshot-metric abstraction for ratchet, not AST compute); (ii) have **only the main crate** as a
  dependent; (iii) **duplicate** `compute_complexity`. It would exist solely to give "metrics" a verb.
- Report structs (`FunctionComplexity`, `RiskLevel`/`LengthCategory` tiers, `FullStats`,
  `FileReport<T>`) + the `OutputFormatter`/`RankEntry` impls are **CLI wiring** — they stay.

**Conclusion:** Metric core **stays A1**. Optional genuine cleanup: lift the per-function
complexity/length tags-walking into `normalize-facts::extract` (it half-lives there already), which
removes a wrapper, not adds a crate. Folding the existing `normalize-metrics` (ratchet `Metric`)
into anything is unrelated. Building `normalize-metrics`-the-AST-crate = manufacturing a crate for a verb.

---

## Candidate 2 — Git-history cluster — **PASSES the bar** (the real seam; two layers)

### Where the compute lives
- `crates/normalize/src/commands/analyze/git_utils.rs` (925 L) — pure **`gix`** read ops.
  Public surface: `open_repo`, `git_head`, `git_head_branch`, `git_commit_timestamps`,
  `git_log_timestamps`/`CommitEntry`, `resolve_ref`, `resolve_merge_base`, `git_show`,
  `git_diff_name_status`/`DiffFileStatus`, `git_ls_files`, `git_remote_origin_url`,
  churn (`git_file_churn_stats`/`FileChurnEntry`), `git_author_commit_counts`,
  `git_activity_commits`, `git_per_commit_files`, `git_last_commit_for_path`,
  `git_commit_count_for_path`, `run_in_worktree`. **Zero main-crate types** (only `gix` + std).
- `git_history.rs` (90 L) — snapshot selection over `git_utils`.
- Analysis commands, each = report struct + `OutputFormatter` + compute, interleaved:
  `hotspots.rs` (`analyze_hotspots`, `hotspot_score`, `parse_git_churn`, `compute_file_complexities`),
  `coupling.rs` (`analyze_coupling` — co-change), `ownership.rs` (`analyze_ownership` + `blame_file` —
  bus-factor), `contributors.rs`, `activity.rs`, `repo_coupling.rs`, `cross_repo_health.rs`.
- **None of these touch the SQLite index** (the lone `index` grep hit in `git_utils` is git's *own*
  index, `repo.open_index()`).

### Layer A — low-level git read ops → `normalize-git`: **PASSES on criterion (a)**
The decisive evidence is **live duplication across the workspace today**:
- `open_repo`, `read_blob_text`, `walk_tree_at_ref`, `traverse_tree_entries` duplicated **verbatim**
  in `crates/normalize-budget/src/git_ops.rs` **and** `crates/normalize-ratchet/src/git_ops.rs`.
- `open_repo` *also* in `crates/normalize-semantic/src/git_staleness.rs`.
- `blame_file` duplicated **within the main crate** — `ownership.rs:143` and `provenance.rs:211`
  (slightly different return types).
- Further `gix::` consumers each rolling their own helpers: `normalize-facts/src/index.rs`,
  `normalize-native-rules/src/{stale_summary,stale_doc}.rs`, `normalize-semantic/src/populate.rs`,
  `normalize/src/commands/view/history.rs`, `normalize/src/commands/analyze/provenance.rs`.

That is **6+ actual dependents** with verbatim copy-paste — exactly criterion (a). A read-only
`normalize-git` crate (open/head/log/diff/blame/churn/blob/tree-walk) is real consolidation that
fixes a present-tense duplication bug, not speculative reuse. Self-contained seam: `git_utils.rs`
moves as-is (no main-crate types). **Low risk, high payoff.**

### Layer B — git-history *analysis* → e.g. `normalize-git-history`: **PASSES on criterion (b)**
Hotspot scoring (churn × complexity), co-change coupling, bus-factor ownership, contributor/activity
cadence, cross-repo health — "derive code-health signals from git history" is a **recognized standalone
tool category** (code-maat, git-of-theseus). Someone would run churn/coupling/bus-factor on their repo
without normalize. That is genuine standalone usefulness in the same class as the sanctioned
`normalize-graph` / `normalize-code-similarity`, not "theoretically reusable someday."

Seam: the compute fns (`analyze_*`, `hotspot_score`, `parse_git_churn`, blame/ownership) are
separable from the report structs + `OutputFormatter`, but **currently interleaved** in each command
file. Extraction requires defining a typed data API (e.g. `ChurnStats`, `CoupledPair`,
`OwnershipEntry`, `HotspotEntry`) and **leaving the `OutputFormatter` impls in the main crate**.
Cross-dependency: `hotspots` consumes the complexity metric (Layer-1 fold-into-facts), so it would
depend on `normalize-facts` — fine, that's the substrate. **Medium effort, medium risk** (API design +
disentangle compute from format; must not drag `OutputFormatter`/`RankEntry`).

### Verdict
- **`normalize-git` (Layer A): PASSES decisively on (a).** Build it; collapse budget/ratchet/semantic/
  native-rules/main duplication into it. This is the strongest single finding in this investigation.
- **`normalize-git-history` (Layer B): PASSES on (b).** Genuine, but it *is* the part that "would also
  make a nice verb" — it survives the skeptic only because the standalone tool category is real.
  Sequence it after Layer A and only if the compute/format disentangling is done honestly.

---

## Candidate 3 — Other clusters — **no new pass**

- **Index-bound analyses** (`surface`, `imports`, `layering`, `architecture`, `liveness`, `exceptions`,
  `depth_map`, `graph`, `call_graph`, `trace`, `coupling_clusters`, `summary`, `provenance`,
  `docs`, `test_gaps`) all consume `crate::index::FileIndex`. Not cleanly extractable without first
  extracting the SQLite index — out of scope, and not a metric/git seam.
- **Already-homed**: architecture/graph → `normalize-architecture` / `normalize-graph`;
  duplicates/fragments/uniqueness → `normalize-code-similarity`; facts → `normalize-facts`. No
  mis-housed compute among these that newly passes the bar.
- No additional natural compute grouping found beyond the git cluster.

---

## Bottom line — how much of "A2" is real architecture vs verb-manufacturing

- **Metric core: ~0 new crates of real architecture.** The only domain-logic extraction is folding
  per-function complexity/length tags-walking into `normalize-facts` (already half there) — a wrapper
  removal, not a crate. A `normalize-metrics`-AST crate would collide with the existing ratchet
  `normalize-metrics`, have one dependent, duplicate `compute_complexity`, and exist only to back a
  verb. **Manufacturing a crate for a verb → stays A1.**
- **Git-history: this is where genuine A2 lives.** A read-only **`normalize-git`** crate passes
  *outright on (a)* — verbatim duplication across budget/ratchet/semantic/native-rules/facts/main is a
  bug today. A **`normalize-git-history`** analysis crate passes on (b) (code-maat category). The
  former is the slam-dunk; the latter is real but is also the part most flattered by the verb taxonomy,
  so it must be justified on the standalone merit alone and only after the low-level layer lands.

So of the metric+git A2 ambition: the metric half is essentially verb-manufacturing (fold one real bit
into facts, keep the rest in `commands/`), and the git half is the legitimate extraction — primarily a
multi-dependent low-level git crate, secondarily a standalone-useful git-history-analysis crate.
