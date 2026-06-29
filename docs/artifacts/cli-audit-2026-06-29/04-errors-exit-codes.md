# CLI Audit: Error Handling & Exit Codes

**Date:** 2026-06-29  
**Auditor:** Claude Code (automated read-only audit)  
**Build:** `cargo build` succeeded (warnings only, no errors)  
**Total invocations sampled:** 42

## Summary

The normalize CLI has correct exit codes for most surface-level error classes (missing args,
invalid flags, nonexistent file paths for single-file commands). The primary defect class is
**silent empty results with exit 0** on index-dependent commands when the import/call-graph
index has not been built. This affects both the human-readable text output (which sometimes
prints an advisory message but still exits 0) and the JSON output (which silently returns
empty collections with no error signal).

A secondary concern is `grep` against a nonexistent path: the path is silently ignored and
the "no matches" message gives no indication the path was invalid.

---

## Test Table

| # | Command | Stdout summary | Stderr summary | Exit | Verdict |
|---|---------|---------------|----------------|------|---------|
| 1 | `normalize --help` (piped) | Full help text | — | 0 | OK |
| 2 | `normalize view src/main.rs` (ambiguous) | "Multiple matches…" list | — | 1 | OK |
| 3 | `normalize view` (no args) | Directory tree listing | — | 0 | OK |
| 4 | `normalize view /nonexistent/path.rs` | "Path not found…" + did-you-mean | — | 1 | OK |
| 5 | `normalize view --invalid-flag src/main.rs` | Clap error | — | 2 | OK |
| 6 | `normalize view crates/normalize/src/main.rs --json` | Valid JSON | — | 0 | OK |
| 7 | `normalize view crates/normalize/src/main.rs/main` | Full source | — | 0 | OK |
| 8 | `normalize syntax ast` (missing arg) | "Missing required argument: file" | — | 1 | OK |
| 9 | `normalize syntax ast /nonexistent/path.rs` | "Failed to read …: No such file…" | — | 1 | OK |
| 10 | `normalize syntax ast crates/normalize/src/main.rs` | Valid JSON AST | — | 0 | OK |
| 11 | `normalize syntax query --help` | Help text | — | 0 | OK |
| 12 | `normalize analyze complexity` (no arg) | "path not found: complexity" | — | 1 | MED |
| 13 | `normalize analyze complexity /nonexistent/path.rs` | Clap error (unexpected arg) | — | 2 | MED |
| 14 | `normalize analyze health` (test dir, no index) | Health report with real data | — | 0 | OK |
| 15 | `normalize analyze health` (main project) | Full health report | — | 0 | OK |
| 16 | `normalize analyze architecture` (no import index) | "HUBS: none… SUMMARY: 2 modules, 0 imports" | — | 0 | MED |
| 17 | `normalize analyze docs` (test dir) | "0% documented" | — | 0 | OK |
| 18 | `normalize analyze security` (test dir) | "0 findings" | — | 0 | OK |
| 19 | `normalize analyze coupling-clusters` (no git) | "co_change_edges empty, fallback… Not a git repo" | — | 1 | OK |
| 20 | `normalize grep` (no args) | "Missing required argument: pattern" | — | 1 | OK |
| 21 | `normalize grep somepattern /nonexistent/path.rs` | — | "No matches found for: somepattern" | 1 | MED |
| 22 | `normalize grep "fn"` (test dir) | 2 matches in 2 files | — | 0 | OK |
| 23 | `normalize doesnotexist` | Clap error + tip | — | 2 | OK |
| 24 | `normalize structure` (test dir) | structure help | — | 0 | OK |
| 25 | `normalize structure stats` (test dir, auto-init) | Stats with 0 symbols | — | 0 | OK |
| 26 | `normalize structure files` (test dir) | src/main.rs, src/lib.rs | — | 0 | OK |
| 27 | `normalize view referenced-by src/lib.rs` (test dir) | "Call graph not indexed…" | — | 1 | OK |
| 28 | `normalize view references src/lib.rs` (test dir) | "Call graph not indexed…" | — | 1 | OK |
| 29 | `normalize view graph` (test dir, no import index) | "0 nodes, 0 edges… No data found. Run `structure rebuild`" | — | **0** | **HIGH** |
| 30 | `normalize view graph --json` (test dir, no import index) | `{"nodes":0,"edges":0,…}` (all zeros) | — | **0** | **HIGH** |
| 31 | `normalize view dependents src/lib.rs` (no import index) | "0 files affected · 0 direct…" | — | **0** | **HIGH** |
| 32 | `normalize view dependents src/lib.rs --json` (no import index) | `{"blast_radius":{"direct_count":0,…}}` | — | **0** | **HIGH** |
| 33 | `normalize view import-path src/lib.rs src/main.rs` (no import index) | "No import path found between…" | — | **0** | **HIGH** |
| 34 | `normalize rank imports` (no import index) | "No import data found. Run `structure rebuild` first." | — | **0** | **HIGH** |
| 35 | `normalize rank imports --json` (no import index) | `{"entries":[],"total_imports":0,…}` | — | **0** | **HIGH** |
| 36 | `normalize rank depth-map` (no import index) | "No import data found. Run `structure rebuild` first." | — | **0** | **HIGH** |
| 37 | `normalize rank depth-map --json` (no import index) | `{"modules":[],"stats":{"max_depth":0,…}}` | — | **0** | **HIGH** |
| 38 | `normalize rank layering` (no import index) | "No import data found. Run `structure rebuild` first." | — | **0** | **HIGH** |
| 39 | `normalize rank call-complexity` (no call index) | Results with "0 unresolved callees" (partial data) | — | **0** | **HIGH** |
| 40 | `normalize rank complexity` (test dir) | Correct results (no index needed) | — | 0 | OK |
| 41 | `normalize rank duplicates` (test dir) | "0 groups" | — | 0 | OK |
| 42 | `normalize rank uniqueness` (test dir) | "100% unique" | — | 0 | OK |

---

## HIGH Findings

### HIGH-1: `view graph` exits 0 with empty/zero data when import index is missing

**Commands:**
```
normalize view graph
normalize view graph --json
```

**Reproduction (from a directory with no import index):**
```sh
mkdir -p /tmp/t/src
printf 'pub fn foo() {}' > /tmp/t/src/lib.rs
cd /tmp/t
normalize view graph
```

**Observed text output (exit 0):**
```
# Module graph — 0 nodes, 0 edges, density 0.000
  0 weakly connected components (largest: 0)
  0 circular-dependency clusters, 0 diamonds, 0 bridges, 0 transitive edges

No data found. Run `normalize structure rebuild` first.
```

**Observed JSON output (exit 0):**
```json
{"bridges":[],"dead_nodes":[],"diamonds":[],"longest_chains":[],"sccs":[],
 "stats":{"bridge_count":0,"chain_count":0,"dead_node_count":0,"density":0.0,
           "diamond_count":0,"edges":0,"largest_component_size":0,"max_chain_depth":0,
           "nodes":0,"nontrivial_scc_count":0,"scc_count":0,"transitive_edge_count":0,
           "weakly_connected_components":0},"target":"modules","transitive_edges":[]}
```

The text output at least prints an advisory. The JSON output gives zero signal to a programmatic consumer that the data is missing — it is indistinguishable from a real codebase with no imports. Exit code must be non-zero when returning known-empty results due to a missing prerequisite.

---

### HIGH-2: `view dependents` exits 0 with empty data when import index is missing

**Commands:**
```
normalize view dependents <file>
normalize view dependents <file> --json
```

**Observed text output (exit 0):**
```
# Dependents of src/lib.rs

0 files affected · 0 direct · 0 transitive · 0 untested · max depth 0
```

**Observed JSON output (exit 0):**
```json
{"blast_radius":{"direct_count":0,"max_depth":0,"transitive_count":0,"untested_count":0},
 "graph_target":"modules","target":"src/lib.rs"}
```

No advisory message at all in text mode — a user or agent reading this would believe lib.rs is truly not imported by anything, when in fact no import data has been indexed.

---

### HIGH-3: `view import-path` exits 0 with "no path found" when import index is empty

**Command:**
```
normalize view import-path src/lib.rs src/main.rs
```

**Observed output (exit 0):**
```
No import path found between src/lib.rs and src/main.rs
```

When the import index has never been built, this message is indistinguishable from "we searched and found no path." The correct exit code for "cannot answer this query because prerequisites are missing" is non-zero.

---

### HIGH-4: `rank imports` exits 0 with empty JSON when import index is missing

**Commands:**
```
normalize rank imports
normalize rank imports --json
```

**Observed text (exit 0):**
```
# Import Centrality (all modules) — 0 modules, 0 imports

No import data found. Run `normalize structure rebuild` first.
```

**Observed JSON (exit 0):**
```json
{"entries":[],"internal_only":false,"total_imports":0,"total_modules":0}
```

Same pattern: text has advisory, JSON is silent, exit 0 in both cases.

---

### HIGH-5: `rank depth-map` and `rank layering` exit 0 with empty JSON on missing import index

Same pattern as HIGH-4. Both commands emit the advisory in text mode but return empty JSON with exit 0. JSON consumers get no signal.

**Commands:**
```
normalize rank depth-map
normalize rank depth-map --json
normalize rank layering
normalize rank layering --json
```

---

### HIGH-6: `rank call-complexity` returns partial/misleading data when call graph is missing

**Command:**
```
normalize rank call-complexity
```

**Observed (exit 0):**
```
# Call Complexity — normalize-audit-test, 2 functions, 0.0% unresolved callees
```

When no call graph has been indexed, all callees are unresolved — but the command reports "0.0% unresolved callees" because no calls exist in the DB. The output looks like a normal result (both functions have "0 reachable", "1x amplification"). This is misleading: the actual inter-function complexity is unknown, not zero.

---

## MED Findings

### MED-1: `grep` against a nonexistent path silently returns "no matches"

**Command:**
```
normalize grep somepattern /nonexistent/path.rs
```

**Observed (exit 1):**
```
# stderr: No matches found for: somepattern
```

The path `/nonexistent/path.rs` does not exist, but the CLI gives no path-not-found error — it silently treats the missing path as an empty search scope. Exit 1 is correct (no matches), but the message is not actionable for an agent that passed a wrong path.

---

### MED-2: `analyze complexity` confusing error on missing subcommand

**Command:**
```
normalize analyze complexity
```

`analyze` does not have a `complexity` subcommand (that lives under `rank`). Passing "complexity" as the positional `[target]` argument gives:

```
path not found: complexity
```

This is technically correct but likely to confuse users coming from other tools. The error does not suggest `normalize rank complexity`.

---

### MED-3: `analyze architecture` shows zeroed-out report instead of advisory when import index is missing

**Command:**
```
normalize analyze architecture
```

**Observed (exit 0):**
```
HUBS: none
LAYERS: none
COUPLING: none
SUMMARY: 2 modules, 2 symbols, 0 imports (0 resolved), 0 cross-imports, 0 orphans
```

No advisory telling the user to rebuild the index. The summary line ("0 imports (0 resolved)") is a weak signal. Compare to `rank imports` which at least prints "No import data found. Run `normalize structure rebuild` first."

---

## Overall Assessment

### Safe for non-interactive/agent use?

**Partially.** The surface-level CLI contract (missing args, invalid flags, nonexistent single-file paths) is clean: these all return non-zero with actionable stderr messages. The `--json` format for single-file commands (`view`, `syntax ast`) works correctly with valid JSON on stdout and errors on stderr.

The **failure mode** is the category of import-graph and call-graph commands: they exit 0 and return empty-looking JSON when the prerequisite index has not been built. An agent pipeline that runs `normalize view graph --json` or `normalize rank imports --json` has no reliable way to distinguish "the codebase has no imports" from "the import index was never built." The advisory text messages (where present) are only in human-readable mode and not machine-parseable.

### Impact matrix

| Severity | Count | Commands affected |
|----------|-------|-------------------|
| HIGH | 6 issue groups | `view graph`, `view dependents`, `view import-path`, `rank imports`, `rank depth-map`, `rank layering`, `rank call-complexity` |
| MED | 3 issue groups | `grep <pat> <nonexistent>`, `analyze complexity` (wrong subcommand), `analyze architecture` (no advisory) |

### Recommended fix pattern

For all import/call-graph commands: when the query returns zero results because the prerequisite table is empty (import count = 0 and the codebase has source files), the command should exit non-zero and — in JSON mode — include an `"error"` or `"requires_index": true` field in the response rather than returning a bare empty collection.
