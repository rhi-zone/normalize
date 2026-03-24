# Polish State

Created: f89f7a3c5d17cb1d8b13137bacb8c830d54c808c
Last run: 2026-03-24T14:50:00Z
Round 1 applied: 2026-03-24
Round: 4 (fixpoint reached)
Project type: Rust CLI + library ecosystem (~40 crates)

## Lenses
- api-clarity
- naming-consistency
- doc-coverage
- error-surface
- adversarial

## Scope
Primary: crates/normalize-metrics/, crates/normalize-ratchet/, crates/normalize-budget/,
crates/normalize/src/commands/ci.rs, crates/normalize-native-rules/src/ratchet.rs,
crates/normalize-native-rules/src/budget.rs

Secondary: entire workspace (spot-checks for consistency)

## Findings — Round 1

### Conflicts
None. Naming-consistency and api-clarity findings on `*Result` vs `*Report` and `BudgetLimits`
field names are complementary, not contradictory.

---

### adversarial

- [DONE] `crates/normalize-ratchet/src/metrics/call_complexity.rs:157-163` — index misalignment between `fn_entries` and `non_container_tis` when `node_name()` returns `None` (causes skip in fn_entries but not non_container_tis), producing wrong TagInfo for call-edge row-range lookup → wrong call graphs and complexity values — add a parallel index into non_container_tis that tracks separately, or filter non_container_tis in lockstep _(severity: high)_

- [DONE] `crates/normalize-metrics/src/filter.rs:13-18` — dead trailing-slash branch: `prefix` is trimmed into a new local `String` but then `prefix.ends_with('/')` checks the original `&str` binding; the documented "trailing-slash matches addr.starts_with(prefix)" behaviour silently never fires — fix to check the trimmed local or restructure the branches _(severity: medium)_ [also flagged by api-clarity]

- [DONE] `crates/normalize-ratchet/src/service.rs:658` — `filter_entries` uses raw `e.path.starts_with(p)` string prefix match; `"src"` matches `"srcother/..."`. Should use `normalize_metrics::filter_by_prefix` _(severity: medium)_

- [DONE] `crates/normalize-ratchet/src/service.rs:445` — `update` has the same raw `starts_with` bug as `filter_entries:658` _(severity: medium)_

- [DONE] `crates/normalize-ratchet/src/service.rs:687-690` — delta `current - baseline` propagates NaN/infinity (e.g. from BFS sum); the `abs() < 1e-10` epsilon check evaluates to `false` for NaN, silently classifying NaN delta as Regression — guard with `is_finite()` before classification _(severity: medium)_

- [DONE] `crates/normalize-ratchet/src/service.rs:754` + `crates/normalize-budget/src/metrics/functions.rs:45` — worktree name uses first 7 chars of hash; two concurrent runs for the same ref race on stale-cleanup + `git worktree add`, causing "already exists" failure — use a unique suffix (pid or tempdir) _(severity: medium)_

- [DONE] `crates/normalize-ratchet/src/service.rs:760-765` + `crates/normalize-budget/src/metrics/functions.rs:51-56` — `git worktree remove --force` failure silently ignored; subsequent `git worktree add` then fails — check exit code and surface error _(severity: medium)_

- [DONE] `crates/normalize-budget/src/service.rs:648-680` — `check_limits` uses plain `>` comparisons; `NaN > max` is `false`, so NaN metric values silently skip all violations — guard with `is_finite()` _(severity: medium)_

- [DONE] `crates/normalize-budget/src/service.rs:618-619` — if path prefix matches nothing, `added`/`removed` are both `0.0` and all limit checks silently pass; a typo in a prefix is invisible — warn when a configured entry matches zero files _(severity: medium)_

- [DONE] `crates/normalize-ratchet/src/metrics/complexity.rs:115` — `?` aborts the entire file's measurements when one function's node can't be found by byte range — change to `continue` to skip just that function _(severity: medium)_

- [DONE] `crates/normalize-budget/src/metrics/complexity_delta.rs:154` — same whole-file abort on first missed node _(severity: medium)_

- [DONE] `crates/normalize-metrics/src/aggregate.rs:57-68` — NaN in inputs silently propagates through Mean, Max, Min; Median sorts NaN as Equal, returning NaN as result — filter or reject NaN inputs before aggregation _(severity: medium)_

- [DONE] Cross-cutting (build_ratchet_diagnostics / build_budget_diagnostics) — git operations outside a git repo return errors that are swallowed with `eprintln!`, causing CI to see a clean pass with no diagnostics when the tool couldn't check anything — return a typed error or produce a diagnostic indicating the failure _(severity: medium)_ [also flagged by error-surface]

- [DONE] `crates/normalize-budget/src/metrics/todos.rs:41` + `crates/normalize-budget/src/metrics/dependencies.rs:88` — `&line[1..]` byte-slices a `&str` at index 1 without verifying it's a char boundary; safe for ASCII diff output but panics on adversarial non-ASCII input — use `line[1..].to_string()` with a char-boundary check or use `chars().skip(1)` _(severity: low)_

- [DONE] `crates/normalize-ratchet/src/service.rs:580` + `crates/normalize-budget/src/service.rs:557` — `resolve_root` silently falls back to `"."` if cwd is unavailable (common in ephemeral CI containers) — surface the error rather than silently substituting _(severity: low)_

- [DONE] `crates/normalize-ratchet/src/metrics/call_complexity.rs:289` — adjacency list in call graph may have duplicate callee entries (no dedup on push at line 229), inflating BFS reachable sum _(severity: low)_

---

### naming-consistency

- [DONE] `crates/normalize-ratchet/src/service.rs:19,182,201` + `crates/normalize-budget/src/service.rs:20,96,172` — `MeasureResult`, `AddResult`, `RemoveResult` should be `MeasureReport`, `AddReport`, `RemoveReport` — all 30+ other commands use `*Report`; the mutation operations in these two services are the only exception _(severity: high)_

- [DONE] `crates/normalize-budget/src/service.rs:111` — `UpdateResult` vs ratchet's `UpdateReport` — same operation, two names within the same crate family — rename budget's `UpdateResult` → `UpdateReport` _(severity: high)_

- [DONE] `crates/normalize-ratchet/src/service.rs:608` + `crates/normalize-budget/src/service.rs:585` — `do_measure` uses a private-helper-convention `do_` prefix despite being `pub` (ratchet) or `pub(crate)` (budget) — rename to `measure` or `ratchet_measure`/`budget_measure` _(severity: medium)_ [also flagged by api-clarity]

- [DONE] `crates/normalize-ratchet/src/service.rs:966` vs `crates/normalize-native-rules/src/ratchet.rs:12` — `build_ratchet_diagnostics` (service layer) vs `build_ratchet_report` (native-rules wrapper) — same call chain, different suffix — align to `build_ratchet_report` at both layers _(severity: medium)_

- [DONE] `crates/normalize-budget/src/service.rs:687` vs `crates/normalize-native-rules/src/budget.rs:12` — same `_diagnostics` vs `_report` mismatch _(severity: medium)_

- [DONE] `crates/normalize-budget/src/metrics/` — metric struct names use plural nouns (`LinesMetric`, `FunctionsMetric`, `ClassesMetric`) vs ratchet's full descriptive names (`LineCountMetric`, `FunctionCountMetric`, `ClassCountMetric`) — consider `LineDeltaMetric`, `FunctionDeltaMetric` etc. to signal diff semantics _(severity: medium)_

- [DONE] `crates/normalize-budget/src/budget.rs:11-17` — `BudgetLimits` fields `added`, `removed`, `total`, `net` vs CLI flags `--max-added`, `--max-removed`, `--max-total`, `--max-net` — struct fields should be `max_added` etc. to match CLI and convey "these are maximums" _(severity: medium)_ [also flagged by api-clarity]

- [DONE] `crates/normalize-ratchet/src/baseline.rs:35` — `baseline_path()` uses `"baseline"` noun but the feature is named `ratchet`; callers would look for `ratchet_path()` — rename _(severity: low)_

- [DONE] `crates/normalize-native-rules/src/ratchet.rs:12` + `budget.rs:12` — these return `DiagnosticsReport` directly while all other native rules return a domain `*Report` type with `OutputFormatter` + `From<> impl` — ratchet/budget skip the intermediate type, breaking the native-rules pattern _(severity: medium)_

---

### api-clarity

- [DONE] `crates/normalize-ratchet/src/service.rs:231` + `crates/normalize-budget/src/service.rs:196` — `::new(pretty: &Cell<bool>)` copies the value at construction time but the parameter type implies it observes future changes — change to `bool` _(severity: high)_

- [DONE] `crates/normalize-metrics/src/aggregate.rs:52` — `aggregate()` function shares a root name with the `Aggregate` enum in scope; call sites read `aggregate(&mut v, Aggregate::Mean)` which is confusing — rename function to `compute_aggregate` or `reduce` _(severity: medium)_

- [DONE] `crates/normalize-metrics/src/aggregate.rs:52` — `aggregate()` mutates input slice for `Median` (sorts in place) but all other strategies are pure; mutation is surprising and undocumented on the signature — take ownership (`Vec<f64>`) or document mutation clearly _(severity: medium)_

- [DONE] `crates/normalize-budget/src/metrics/mod.rs:27` — `DiffMetric::measure_diff` returns `Vec<(String, f64, f64)>` anonymous 3-tuple; callers use `.1` / `.2` with no hint of which is `added` vs `removed` — introduce `DiffMeasurement { key: String, added: f64, removed: f64 }` _(severity: medium)_

- [DONE] `crates/normalize-ratchet/src/service.rs:46` — `CheckEntry.aggregate` is `String` while `BaselineEntry.aggregate` is `Aggregate` — same field, different representation; consumers re-parsing the string back to enum — use `Aggregate` on `CheckEntry` _(severity: medium)_

- [DONE] `crates/normalize-ratchet/src/service.rs:116-123` — `UpdateEntry.reason: String` with hardcoded values `"forced"`, `"improved"`, `"no improvement"` — downstream matchers are fragile string comparisons — introduce `UpdateReason` enum _(severity: medium)_

- [DONE] `crates/normalize-ratchet/src/baseline.rs:35-37` + `crates/normalize-budget/src/budget.rs:64` — `load_baseline`, `save_baseline`, `baseline_path` are free functions rather than `BaselineFile` methods; not discoverable via IDE autocomplete on the type — convert to `BaselineFile::load(root)` / `baseline.save(root)` etc. _(severity: medium)_

- [DONE] `crates/normalize-ratchet/src/service.rs:278` + budget equivalent — `aggregate: Option<String>` parameter forces callers to pass unvalidated string; `Aggregate` has `FromStr` — accept `Option<Aggregate>` instead _(severity: medium)_

- [DONE] `crates/normalize/src/commands/ci.rs:18-31` — `CiReport` stores `error_count`/`warning_count`/`info_count` as fields derivable from `diagnostics`; caller who modifies `diagnostics` after construction has stale counts — make them accessor methods _(severity: medium)_

- [DONE] `crates/normalize-budget/src/service.rs:96-107` (AddResult/UpdateResult/RemoveResult) — `message: String` field duplicates information already in structured fields; `format_text()` can construct it — remove the field _(severity: medium)_

- [DONE] `crates/normalize-ratchet/src/lib.rs:18` — `default_metrics(root: &Path)` ignores `root`; parameter implies root-specific metrics but all returned metrics are stateless — remove the parameter or document that it is unused _(severity: medium)_

- [DONE] `crates/normalize-metrics/src/filter.rs:9` — returns `impl Iterator<Item = &'a (String, f64)>` (tuple); no compile-time documentation of which is address vs value — introduce `MetricPoint { address: String, value: f64 }` _(severity: medium)_

- [DONE] `crates/normalize-ratchet/src/baseline.rs:40` — `load_baseline` returns `Ok(BaselineFile::default())` for missing file; callers can't distinguish "never initialised" from "no entries" — consider `Ok(None)` / `Result<Option<BaselineFile>>` _(severity: low)_

---

### error-surface

- [DONE] `crates/normalize-metrics/src/lib.rs:32` + `crates/normalize-budget/src/metrics/mod.rs:27` — `Metric::measure_all` and `DiffMetric::measure_diff` return `anyhow::Result` in public library traits — consumers can't match on failure modes; replace with typed error enums _(severity: high)_

- [DONE] Cross-cutting — no structured error types in any of the three new crates; all errors are `anyhow::Error` at trait boundary or `String` in service methods — library callers (CI tools) cannot programmatically distinguish "baseline not found" vs "JSON parse error" vs "metric unknown" — add `thiserror`-defined error enums _(severity: high)_

- [DONE] `crates/normalize-budget/src/service.rs:567-574` — `load_budget_config` silently discards both read errors and TOML parse errors (`unwrap_or_default()`); malformed `[budget]` config section is invisible to the user — surface as warning or error _(severity: medium)_

- [DONE] `crates/normalize-ratchet/src/service.rs:974-989` + `crates/normalize-budget/src/service.rs:693-695` — `load_baseline` / `load_budget` errors silently swallowed (eprintln + empty report); corrupt file is indistinguishable from absent file at the caller — return typed result or produce a diagnostic _(severity: medium)_ [also flagged by adversarial]

- [DONE] `crates/normalize-ratchet/src/service.rs:621` + `crates/normalize-budget/src/service.rs:599` — `do_measure` error converted with `.map_err(|e| e.to_string())`; root path and metric name absent from resulting string — include context in the error string _(severity: medium)_

- [DONE] `crates/normalize-ratchet/src/baseline.rs:54-55` + `crates/normalize-budget/src/budget.rs:79` — `fs::create_dir_all(parent)?` propagates bare `io::Error` with no path — wrap with `.with_context(|| format!("creating {}", parent.display()))` _(severity: medium)_

- [DONE] `crates/normalize-ratchet/src/service.rs:795` — `.unwrap()` on `metrics.iter().find(...)` — safe in practice but would panic if MetricFactory returns a metric whose `name()` changes — guard with `.ok_or_else(...)` _(severity: medium)_

---

### doc-coverage

- [DONE] `crates/normalize-ratchet/src/baseline.rs:11` — `BaselineEntry` fields (`path`, `metric`, `aggregate`, `value`) have no doc comments — add per-field docs explaining semantics _(severity: high)_

- [DONE] `crates/normalize-ratchet/src/service.rs:608` — `pub fn do_measure` has no doc comment despite being the core computation function called by CLI and rules engine _(severity: high)_

- [DONE] `crates/normalize-budget/src/service.rs:19` — `MeasureResult` fields `total`, `net`, `item_count` have no docs; `total = added + removed`, `net = added - removed` are non-obvious _(severity: high)_

- [DONE] `crates/normalize-ratchet/src/service.rs:19-56` — `MeasureResult`, `CheckReport`, `CheckEntry`, `CheckStatus` fields all undocumented _(severity: medium)_

- [DONE] `crates/normalize-ratchet/src/service.rs:116-123` — `UpdateEntry.reason` field undocumented; valid values are opaque _(severity: medium)_

- [DONE] `crates/normalize-ratchet/src/service.rs:231` — `RatchetService::new()` and `::with_factory()` have no doc comments _(severity: medium)_

- [DONE] `crates/normalize-budget/src/service.rs:51,66` — `CheckEntry.violations` and `CheckReport` fields undocumented _(severity: medium)_

- [DONE] `crates/normalize-budget/src/service.rs:196` — `BudgetService::new()` and `::with_factory()` have no doc comments _(severity: medium)_

- [DONE] `crates/normalize-ratchet/src/metrics/call_complexity.rs:15` — `CallComplexityMetric` doc omits BFS cycle handling and cross-file resolution strategy _(severity: medium)_

- [DONE] `crates/normalize-ratchet/src/lib.rs:18` — `default_metrics()` has no doc comment explaining which metrics are included _(severity: medium)_

- [DONE] `crates/normalize-metrics/src/aggregate.rs:10` — `Aggregate` enum variants have no doc comments _(severity: medium)_

- [DONE] `crates/normalize-budget/src/budget.rs:43` — `BudgetFile` fields undocumented _(severity: medium)_

- [DONE] `crates/normalize-ratchet/src/baseline.rs:20` — `BaselineFile` fields undocumented _(severity: medium)_

- [DONE] `crates/normalize-budget/src/service.rs:585` — `do_measure()` (pub(crate)) has no doc comment _(severity: medium)_

- [DONE] `crates/normalize-ratchet/src/service.rs:199` — `RemoveResult.removed: bool` has no doc comment _(severity: low)_

- [DONE] `crates/normalize-budget/src/budget.rs:21` — `BudgetLimits::is_empty()` has no doc comment _(severity: low)_

- [DONE] `crates/normalize-budget/src/budget.rs:99` — `BudgetConfig::effective_ref()` and `::effective_aggregate()` have no doc comments _(severity: low)_

---

## Findings — Round 2

Round 2 git hash: 036706ad
Scope: normalize-languages, normalize-surface-syntax, normalize-language-meta, normalize-grammars,
normalize-facts/facts-core/facts-rules-*, normalize-rules/rules-config/syntax-rules/native-rules,
normalize-core/graph/analyze/architecture/output/filter/scope/path-resolve/manifest/deps,
normalize crate (main), normalize-edit/shadow/tools/ecosystems/typegen/openapi/code-similarity/
session-analysis/chat-sessions/package-index/local-deps

### Conflicts
None.

---

### adversarial

- [DONE] `crates/normalize-analyze/src/lib.rs:63` — `truncate_path` slices by raw byte offset; panics on non-ASCII input (char boundary) AND panics when `max_len < 4` (usize underflow) — guard `max_len >= 4`, use `char_indices` to find safe boundary _(severity: high)_

- [DONE] `crates/normalize-architecture/src/lib.rs:303` — `symbol_count as usize` from `i64` DB column without negative check; `-1i64 as usize` = `usize::MAX` — guard with `if symbol_count < 0 { 0 } else { symbol_count as usize }` _(severity: high)_

- [DONE] `crates/normalize-architecture/src/lib.rs:366` — same `count as usize` from `i64` in `find_symbol_hotspots` _(severity: high)_

- [DONE] `crates/normalize-languages/src/grammar_loader.rs:406` — `Library::new(path).ok()?` silently discards library load errors; ABI mismatches are invisible — log at debug level before returning None _(severity: high)_ [linked to error-surface finding below]

- [DONE] `crates/normalize-languages/src/grammar_loader.rs:152` — `RwLock::read().ok()?` silently returns None on lock poison; all subsequent queries appear as "grammar not found" — use `unwrap_or_else(|e| e.into_inner())` to recover from poisoned lock _(severity: medium)_

- [DONE] `crates/normalize-languages/src/scss.rs:35`, `dart.rs:129`, `svelte.rs:38` — `rest.chars().nth(start)` where `start` is a byte offset from `str::find`; wrong for multi-byte UTF-8 before the quote char; panic on `unwrap()` — use `rest[start..].chars().next()` _(severity: medium)_

- [DONE] `crates/normalize/src/commands/analyze/ownership.rs:166` — `sorted[0].1 as f64 / total_lines as f64` panics/produces NaN when `total_lines == 0`; map non-empty but all zero counts is a reachable edge case — guard with `if total_lines == 0 { return None; }` _(severity: medium)_

- [DONE] `crates/normalize/src/service/edit.rs:420,613,768,858,1380` — `std::env::current_dir().unwrap()` panics if cwd is deleted (common in CI); propagate as error _(severity: low)_

- [DONE] `crates/normalize/src/service/mod.rs:119` — `current_dir().unwrap_or_default()` falls back to empty PathBuf `""`, not `"."`, silently wrong — change to `unwrap_or_else(|_| PathBuf::from("."))` _(severity: low)_

- [DONE] `crates/normalize-architecture/src/lib.rs:352-391` — `find_symbol_hotspots` issues N+1 SQL prepares inside a loop (one per symbol name) — rewrite with `WHERE name IN (...)` _(severity: medium)_

- [DONE] `crates/normalize-graph/src/lib.rs:188,197,212,221` — raw ANSI escape codes (`\x1b[1;36m` etc.) hardcoded in `DependentsReport::format_text` while rest of codebase uses `nu_ansi_term` — replace with `nu_ansi_term` calls _(severity: medium)_

- [DONE] `crates/normalize-output/src/lib.rs:112` — `"░".repeat(width - filled)` has no guard that `filled <= width`; floating-point rounding could produce `filled > width` — add `let filled = filled.min(width)` _(severity: low)_

- [DONE] `crates/normalize-native-rules/src/stale_docs.rs:49` — `(cover.code_modified - doc.doc_modified) / 86400` unchecked subtraction on `u64`; filter guards it but use `.saturating_sub()` to be explicit _(severity: low)_

---

### error-surface

- [DONE] `crates/normalize-languages/src/grammar_loader.rs:~406` — `get()` returns `Option<Language>` with no way to distinguish "not found" / "load failed" / "ABI incompatible"; change to `Result<Option<Language>, GrammarLoadError>` _(severity: high)_

- [DONE] `crates/normalize-facts-rules-api/src/rule_pack.rs:53-69` — `RulePack::run` returns `RVec<Diagnostic>` with no error signal; a panicking dylib unwinds across the FFI boundary (UB); document no-panic contract or add `catch_unwind` in the loader _(severity: high)_

- [DONE] `crates/normalize-facts-rules-interpret/src/lib.rs:601,629` — execution-time errors from `engine.run()` / `run_incremental()` are mapped to `InterpretError::Parse`; add `InterpretError::Eval(String)` variant _(severity: medium)_

- [DONE] `crates/normalize-filter/src/lib.rs:196-200` — `Filter::new` returns `Result<Self, String>`; define `FilterError` enum _(severity: medium)_

- [DONE] `crates/normalize-manifest/src/lib.rs:112-118` — `ManifestError(pub String)` implements `Display` but not `std::error::Error`; cannot be used with `?` in standard error chains _(severity: medium)_

- [DONE] `crates/normalize-languages/src/external_packages.rs:179-197` — `PackageIndex::open/open_in_memory` return `Result<_, libsql::Error>` leaking storage impl; wrap in `PackageIndexError` _(severity: medium)_

- [DONE] `crates/normalize/src/commands/tools/lint.rs:238,243` — `build_lint_run` / `build_lint_run_multi` return `Result<_, String>`; define `LintError` _(severity: medium)_

- [DONE] `crates/normalize-shadow/src/lib.rs:1014` — `ShadowError` variants carry only `String`; use structured variants for Init/Commit/Undo/Validation errors _(severity: low)_

---

### naming-consistency

- [DONE] Cross-cutting — `Severity` enum defined independently in `normalize-syntax-rules` and `normalize-facts-rules-interpret` (identical structure, no shared source); `DiagnosticLevel` in `normalize-facts-rules-api` is a third type with a `Hint` variant the others lack; the `Severity::Info → DiagnosticLevel::Hint` mapping at `normalize-facts-rules-interpret:514` is lossy and undocumented — extract shared `Severity` to `normalize-rules-config` _(severity: high)_

- [DONE] `crates/normalize/src/` (multiple files) — 15+ `*Result` output structs should be `*Report`: `LintListResult`, `LintRunResult`, `GrepResult`, `RebuildResult` (facts), `PackagesResult`, `CommandResult`, `QueryResult`, `DaemonActionResult`, `DaemonRootResult`, `PackageResult`, `GenerateResult`, `GrammarInstallResult`, `InitResult`, `UpdateResult`, `TranslateResult` _(severity: medium)_

- [DONE] `crates/normalize-facts/src/service.rs:19-63` — `RebuildResult`, `StatsResult`, `FileList` implement `Display` instead of `OutputFormatter`; inconsistent with every other output struct in workspace _(severity: medium)_

- [DONE] `crates/normalize-syntax-rules/src/service.rs:19-50` — `FindingItem` and `RunResult` don't follow `*Report` convention _(severity: medium)_ [also: `RunResult` has no Output Formatter, uses Display]

- [DONE] `crates/normalize-rules/src/service.rs:111` — `RuleResult` doesn't follow `*Report` convention; also has a dead `data: Option<serde_json::Value>` field with no consumer _(severity: low)_

- [DONE] `crates/normalize-graph/src/lib.rs:105-110` + `crates/normalize-architecture/src/lib.rs:74-81` — `ImportChain` declared twice (identical struct); `find_longest_chains` also duplicated with hardcoded limit 5 in architecture — architecture crate should use normalize-graph's type _(severity: medium)_

- [DONE] `crates/normalize-facts-rules-interpret/src/lib.rs:71-74` — `FactsRuleOverride` / `FactsRulesConfig` stale backward-compat type aliases; comment says they should be removed — remove them _(severity: medium)_

- [DONE] `crates/normalize-language-meta/src/capabilities.rs:39-46` — `Capabilities::none()` and `Capabilities::data_format()` return identical structs (all fields `false`); callers can't tell if there's a semantic difference — either make them distinct or keep only one _(severity: medium)_

---

### api-clarity

- [DONE] `crates/normalize-path-resolve/src/lib.rs:9` — `PathMatch::kind: String` with two hardcoded values `"file"`/`"directory"` compared by string equality in 5+ places — convert to enum `PathMatchKind` _(severity: medium)_

- [DONE] `crates/normalize-path-resolve/src/lib.rs:24-25` — `PathSource::all_files` returns `Option<Vec<(String, bool)>>`; the `bool` means `is_directory` but is undocumented — introduce `PathEntry { path: String, is_dir: bool }` _(severity: medium)_

- [DONE] `crates/normalize-output/src/diagnostics.rs:333` — `"... {} more not shown (use --limit or --pretty to see all)"` embeds CLI flag names in a library crate — the string should not know about CLI flags _(severity: medium)_

- [DONE] `crates/normalize-rules/src/runner.rs` (`show_rule`, `list_tags`, `enable_disable`) — these `pub fn`s print directly to stdout via `println!()` and return `Result<(), String>`; untestable; every other command returns a report struct — return formatted string or structured report _(severity: medium)_

- [DONE] `crates/normalize-rules/src/runner.rs:216-240` — `RulesListReport` embeds rendering flags `sources: bool`, `no_desc: bool` in the data struct; these are CLI output hints, not data — move to `OutputFormatter` call site _(severity: medium)_

- [DONE] `crates/normalize-rules-config/src/lib.rs:39-56,76-96` — `RuleOverride::merge` and `RulesConfig::merge` have asymmetric semantics (Vec: non-empty wins; HashMap: union); impossible to reset a Vec to empty via merge; undocumented — document the semantics explicitly _(severity: medium)_

- [DONE] `crates/normalize-languages/src/external_packages.rs:64-75` — `version_cmp` free function does the same thing as `Version::partial_cmp`; remove the free function _(severity: medium)_

- [DONE] `crates/normalize/src/service/facts.rs:823`, `sessions.rs:64`, `mod.rs:174` — `#[serde(untagged)]` enums produce JSON that's opaque to programmatic consumers and loses variant info in JSON Schema — use `#[serde(tag = "kind")]` _(severity: medium)_

- [DONE] `crates/normalize-facts-rules-builtins/src/circular_deps.rs:51-57` — local-vs-external import heuristic is hard-coded and undocumented; silently misclassifies npm scoped packages, Go module paths, `./relative` imports — document known gaps and add tests _(severity: medium)_

- [DONE] `crates/normalize-facts-rules-api/src/rule_pack.rs:56` — `#[sabi(missing_field(panic))]` causes panics on ABI version mismatch with no documentation for dylib authors — add comment explaining ABI versioning contract _(severity: medium)_

- [DONE] `crates/normalize-tools/src/tools.rs:163-193` — `find_js_tool`/`find_python_tool` return `Option<(String, Vec<String>)>` (anonymous tuple) — introduce `ToolInvocation { command: String, args: Vec<String> }` _(severity: low)_

- [DONE] `crates/normalize-deps/src/lib.rs:96-107` — `DepsExtractor` is a zero-field struct with no doc and no state; replace with free function `pub fn extract(path, content)` _(severity: low)_

---

### doc-coverage

- [DONE] `crates/normalize-facts-core/src/symbol.rs:66-96` — `Symbol` and `FlatSymbol` fields all undocumented; `attributes` format, `implements` vs `is_interface_impl` semantics are non-obvious _(severity: medium)_

- [DONE] `crates/normalize-rules/src/runner.rs:450-458` — `ListFilters` struct and fields undocumented; interaction between `enabled`/`disabled` flags is non-obvious _(severity: low)_

- [DONE] `crates/normalize-native-rules/src/lib.rs:22-27` — `NativeRuleDescriptor` struct undocumented; `default_severity` vs runtime-configured severity lifecycle unexplained _(severity: low)_

- [DONE] `crates/normalize-syntax-rules/src/lib.rs:81-116` — `Rule` struct fields undocumented; `fix` substitution syntax (`$capture`) documented in crate doc but not on the field _(severity: low)_

- [DONE] `crates/normalize-syntax-rules/src/runner.rs:11-28` — `Finding` fields undocumented; `captures` key semantics (whether `@match` is included) matter for fix substitution _(severity: low)_

- [DONE] `crates/normalize-path-resolve/src/lib.rs:1` — missing crate-level `//!` doc _(severity: low)_

- [DONE] `crates/normalize-facts-core/src/symbol.rs:6` — `SymbolKind` enum variants undocumented; `Heading` variant is non-obvious (Markdown) _(severity: low)_

- [DONE] `crates/normalize-facts-core/src/symbol.rs:66-80` — `Symbol` fields undocumented _(severity: medium)_

- [DONE] `crates/normalize-languages/src/grammar_loader.rs:62` — `GrammarLoader` struct has no doc comment _(severity: low)_

- [DONE] `crates/normalize-edit/src/lib.rs:1` — missing crate-level `//!` doc _(severity: low)_

- [DONE] `crates/normalize-facts-rules-api/src/relations.rs` — various `Relations` fields undocumented _(severity: low)_

---

## Findings — Round 3

Round 3 git hash: 001a3e55
Scope: entire codebase — re-audit after Round 2 changes

### Conflicts
None.

---

### adversarial + error-surface

- [DONE] `crates/normalize-ratchet/src/service.rs:880` — `check_against_ref` creates a git worktree and returns early on unknown metric name; worktree is never removed — add a RAII guard that calls `remove_worktree` on drop _(severity: high)_

- [DONE] `crates/normalize-languages/src/grammar_loader.rs:176-192` — `get()` doc says `Ok(None)` is never returned but the return type is `Result<Option<Language>, _>`; 25+ callers use `.ok().flatten()` which silently discards `GrammarLoadError` — change return type to `Result<Language, GrammarLoadError>` _(severity: medium)_

- [DONE] `crates/normalize/src/service/history.rs:71,99,123,158,186`, `syntax.rs:80`, `rank.rs:53`, `analyze.rs:40`, `view.rs:33` — nine `current_dir().unwrap()` calls not fixed in Round 2 (only `edit.rs` was fixed) — propagate error as done in `edit.rs` _(severity: medium)_

- [DONE] `crates/normalize-ratchet/src/error.rs`, `crates/normalize-budget/src/error.rs` — `RatchetError` and `BudgetError` defined and exported but never used; all service methods return `Result<_, String>` — adopt as service return type or remove until needed _(severity: medium)_

- [DONE] `crates/normalize/src/commands/tools/lint.rs:307,260` — `LintError` enum defined but never used; `build_lint_run`/`build_lint_run_multi` still return `Result<_, String>` — use `LintError` as the return type _(severity: medium)_

- [DONE] `crates/normalize-budget/src/metrics/complexity_delta.rs:22-24`, `functions.rs:143-145` — worktree created then `collect_complexity` called; panic between create and remove leaves orphaned worktree — RAII guard _(severity: low)_

- [DONE] `crates/normalize-facts/src/index.rs:1872,1966` — `ProgressStyle::with_template(...).unwrap()` in production `rebuild()` path — use `.unwrap_or_default()` _(severity: low)_

- [DONE] `crates/normalize-ratchet/src/service.rs:962` — `check_against_ref` hardcodes `Aggregate::Mean` in `CheckEntry`; actual aggregate strategy not reflected in report — pass through from caller _(severity: low)_

---

### api-clarity + naming-consistency

- [DONE] `crates/normalize-languages/src/grammar_loader.rs:49-73` — `GrammarLoadError::AbiIncompatible` variant defined but never constructed — remove or implement ABI checking _(severity: medium)_

- [DONE] `crates/normalize-path-resolve/src/lib.rs:22-26` — `PathEntry::is_dir: bool` inconsistent with `PathMatch::kind: PathMatchKind`; callers must manually map between the two — align `PathEntry` to use `kind: PathMatchKind` _(severity: medium)_

- [DONE] `crates/normalize/src/service/facts.rs:824`, `mod.rs:177`, `sessions.rs:65` — `FactsStatsOutput`, `ContextOutput`, `PlansOutput` use `Output` suffix not `Report` — rename to `FactsStatsReport`, `ContextReport` (or `ContextVariantsReport`), `PlansReport` _(severity: medium)_

- [DONE] `crates/normalize/src/service/facts.rs:37,166,186,213` — `RebuildReport`, `FileList`, `PackagesReport`, `CommandReport`, `FactsStats`, `StorageReport` implement `Display` but not `OutputFormatter`; returned from `#[cli]` methods _(severity: medium)_

- [DONE] `crates/normalize/src/service/mod.rs:182,191,216` — `InitReport`, `UpdateReport`, `TranslateReport` implement `Display` but not `OutputFormatter` _(severity: medium)_

- [DONE] `crates/normalize/src/service/grammars.rs`, `generate.rs`, `daemon.rs` — `GrammarInstallReport`, `GenerateReport`, `DaemonActionReport`, `DaemonRootReport`, `DaemonRootList` implement `Display` but not `OutputFormatter` _(severity: medium)_

- [DONE] `crates/normalize-rules/src/service.rs:109-115` — `RuleShowReport` implements `Display` but not `OutputFormatter`; returned from 8 `#[cli]` methods _(severity: medium)_

- [DONE] `crates/normalize/src/commands/tools/test.rs:20` — `TestListResult` should be `TestListReport`; uses `Display` not `OutputFormatter` _(severity: low)_

- [DONE] `crates/normalize/src/commands/tools/test.rs:45` — `RepoLintResult` and `RepoTestResult` should be `RepoLintEntry`/`RepoTestEntry` (suffix `Result` implies `std::result::Result`) _(severity: low)_

- [DONE] `crates/normalize/src/output.rs` — missing `assert_output_formatter` for: `ScalarTrendReport`, `HealthReport`, `RulesListReport`, `RulesValidateReport`, `BudgetDiagnosticsReport`, `RatchetDiagnosticsReport`, `CheckExamplesReport`, `CheckRefsReport`, `StaleDocsReport`, `StaleSummaryReport` _(severity: low)_

- [DONE] `crates/normalize-ratchet/src/error.rs:15`, `crates/normalize-budget/src/error.rs:19` — `RatchetError::MeasurementFailed.reason` vs `BudgetError::MeasurementFailed.message` — inconsistent field name for same semantic concept _(severity: low)_

---

### doc-coverage

- [DONE] `crates/normalize-ratchet/src/service.rs:202` — `ShowEntry` struct missing doc comment _(severity: medium)_

- [DONE] `crates/normalize-ratchet/src/service.rs:658,728` — `filter_entries` and `build_check_report` internal helpers missing doc comments _(severity: medium)_

- [DONE] `crates/normalize/src/commands/tools/lint.rs:33,157,180` — `ToolListItem`, `RepoLintResult` (→`RepoLintEntry`), `LintDiagnostic` fields undocumented _(severity: medium)_

- [DONE] `crates/normalize-path-resolve/src/lib.rs:15,38-40` — `PathMatch` missing doc comment; `PathSource::find_like` and `all_files` methods missing contract docs _(severity: medium)_

- [DONE] `crates/normalize-facts-rules-api/src/lib.rs:23` — `VisibilityFact`, `AttributeFact`, `ParentFact` and 5 other public types not re-exported from `lib.rs` — extend `pub use relations::` or add comment explaining intent _(severity: medium)_

---

## Findings — Round 4

Round 4 git hash: 7726201d
Scope: entire codebase — final fixpoint verification

### Result: FIXPOINT REACHED ✓

Round 4 audit found 3 findings (all low/medium), applied immediately:

- [DONE] `crates/normalize/src/commands/tools/test.rs:67` — `TestRunResult` missed in Round 3 rename sweep — renamed to `TestRunReport` _(severity: medium)_
- [DONE] `crates/normalize/src/output.rs` — missing `assert_output_formatter::<TestRunReport>()` and `assert_output_formatter::<LintRunReport>()` _(severity: low)_
- [DONE] `crates/normalize/src/commands/tools/lint.rs:67-70` — `writeln!().unwrap()` on infallible String writes — replaced with `let _ = writeln!(...)` _(severity: low)_

All lenses return 0 new findings after Round 4 fixes. Polish pass complete.

---

## Findings — Round 5 (per-crate-group deep dive)

Round 5 git hash: 62d11f4d
Scope: four crate groups audited in parallel at depth

### Conflicts
None.

---

### adversarial (high)

- [DONE] `crates/normalize-path-resolve/src/lib.rs:128-145` — `resolve_unified` alias expansion can cycle (a→b→a) causing unbounded recursion/stack overflow — add `depth: u8` limit _(severity: high)_
- [DONE] `crates/normalize-languages/src/rust.rs:160` — `extract_imports` uses `find('}')` which finds the **first** `}`, corrupting nested group imports like `use std::{io::{Read, Write}, fs}` — use brace-depth counter _(severity: high)_
- [DONE] `crates/normalize-architecture/src/lib.rs:448-477` — `find_cycles_dfs` is recursive with no depth limit; large codebases with long import chains will stack-overflow — convert to iterative DFS as other graph algorithms do _(severity: high)_
- [DONE] `crates/normalize-facts/src/index.rs:2299-2306` — `find_like` caps `parts` to 4 AFTER constructing SQL conditions for all parts; when len > 4 the WHERE clause references `?1..?N` but only 4 params are bound → libsql panic or mis-bind — cap `parts` before building `conditions` _(severity: high)_
- [DONE] `crates/normalize-facts-rules-builtins/src/lib.rs:27-43` — `run()` and `run_rule()` are `#[sabi_extern_fn]` (FFI) but do not call `catch_unwind`; the `rule_pack.rs` doc explicitly requires this — wrap in `std::panic::catch_unwind` _(severity: high)_
- [DONE] `crates/normalize-graph/src/lib.rs:~1023` — private `truncate_path` copy uses the old byte-slice version (`&path[path.len() - (max_len - 3)..]`) that panics on multi-byte UTF-8; the fixed version is in `normalize-analyze` — call `normalize_analyze::truncate_path` instead _(severity: high)_

---

### adversarial (medium)

- [DONE] `crates/normalize-rules/src/runner.rs:703,715` — `enable_disable` calls `.as_table_mut().unwrap()` on a newly inserted TOML value; panics when existing `rules` is an inline table, not a `[rules]` section — check for inline table, return `Err` _(severity: medium)_
- [DONE] `crates/normalize-native-rules/src/stale_docs.rs:113,136` — `SystemTime::duration_since(UNIX_EPOCH).unwrap()` panics for pre-epoch mtimes (FAT, network FS, wrong VM clock) — use `.unwrap_or(0)` _(severity: medium)_
- [DONE] `crates/normalize-rules/src/service.rs:279-307` — fix `--fix` loop has no iteration cap; a rule whose fix generates output that still matches the same rule runs forever — add max iteration limit (e.g. 100) _(severity: medium)_
- [DONE] `crates/normalize-languages/src/registry.rs:22,234,249,320` — four `LANGUAGES.read/write().unwrap()` calls; a panicking writer poisons the lock and crashes all subsequent readers — use `unwrap_or_else(|e| e.into_inner())` _(severity: medium)_

---

### adversarial (low)

- [DONE] `crates/normalize-languages/src/svelte.rs:35-38`, `scss.rs:35`, `dart.rs:129` — `.chars().next().unwrap()` (already noted in R2, but three new sites in extract_imports) — use `if let Some(quote)` _(severity: low)_
- [DONE] `crates/normalize-tools/src/custom.rs:111-154` — `Box::leak` in `CustomTool::new` called on every `registry_with_custom` invocation; repeated calls leak strings — cache the registry or use `Arc<str>` _(severity: low)_

---

### api-clarity + naming-consistency (high)

- [DONE] `crates/normalize-rules/src/runner.rs:1638` — `sha256_hex` uses `DefaultHasher` (64-bit non-cryptographic), not SHA-256; the lock file field is named `sha256` — rename to `content_hash` to avoid the lie _(severity: high)_
- [DONE] `crates/normalize-rules/src/runner.rs:1067-1161` — `global_allow` patterns applied to syntax/fact rules but NOT to native-rule issues; users who set `global_allow = ["**/vendor/**"]` expect it to suppress all engines — apply filtering in `apply_native_rules_config` _(severity: high)_
- [DONE] `crates/normalize-rules/src/service.rs:688-728` — `load_rules_config` uses either global OR project `rules` map (not a merge); any non-empty project config silently drops all global per-rule overrides — use `global.merge(project)` _(severity: high)_

---

### api-clarity + naming-consistency (medium)

- [DONE] `crates/normalize-rules/src/service.rs:681` — `validate()` returns `Err(report.format_text())` on invalid config, discarding structured `RulesValidateReport` — return `Ok(report)` and let callers check `report.valid` _(severity: medium)_
- [DONE] `crates/normalize-rules/src/runner.rs:1154` — `RuleType::All` silently excludes SARIF tools from `run_rules_report`; CLI help implies `all` includes all engines — document or include SARIF in `All` _(severity: medium)_
- [DONE] `crates/normalize-rules/src/runner.rs:1027-1065` — `Hint` severity accepted by native-rule config but silently dropped by `normalize_rules_config::Severity::from_str` for syntax/fact rules — add `Hint` to `Severity` enum or validate and error _(severity: medium)_
- [DONE] `crates/normalize-graph/src/lib.rs:920` — `analyze_graph_data(limit=0)` truncates all result Vecs to empty while stats counts retain pre-truncation values; produces reports saying "42 diamonds" with empty `diamonds` list — document that `0` = no limit or treat it as `usize::MAX` _(severity: medium)_
- [DONE] `crates/normalize-facts/src/index.rs:911-952` — `index_file_symbols` takes `calls: &[(String, String, usize)]` (no qualifier) while `reindex_files` uses the richer 4-tuple; qualifier data is silently dropped for calls indexed via public API — add qualifier to the public tuple _(severity: medium)_
- [DONE] `crates/normalize-facts/src/index.rs:1279-1299` — `all_imports()` converts NULL module to `""` silently; callers cannot distinguish "empty module string" from "module was NULL" — return `Option<String>` _(severity: medium)_
- [DONE] `crates/normalize-facts-rules-api/src/relations.rs:44-51` — `ImportFact.to_module` holds raw unresolved specifier in some languages and "" in others; ambiguous between logical name and file path — rename to `module_specifier`, document contract _(severity: medium)_
- [DONE] `crates/normalize-session-analysis/src/lib.rs:111-132` — `ModelPricing::from_model_str` maps all `sonnet-*` to `SONNET_4_5` pricing; claude-sonnet-3-5 sessions get wrong price — add per-version constants _(severity: medium)_
- [DONE] `crates/normalize-session-analysis/src/lib.rs:354-384` — `SessionAnalysis` is both data model and report; workspace convention is `*Report` for service return types — rename to `SessionAnalysisReport` _(severity: medium)_
- [DONE] `crates/normalize-output/src/diagnostics.rs:107-108` — `DiagnosticsReport::merge()` uses `max(files_checked)` not sum; merging two 100-file reports claims 100 checked — use sum _(severity: medium)_
- [DONE] `crates/normalize-tools/src/adapters/eslint.rs:184` — `fix()` swallows JSON parse failure with `.unwrap_or_default()` while `run()` returns `Err(ToolError::ParseError)` for same — return `Err` consistently _(severity: medium)_
- [DONE] `crates/normalize-tools/src/adapters/eslint.rs:193` — `fix()` hardcodes remaining diagnostics to `Warning`; `run()` maps severity properly — apply same mapping _(severity: medium)_
- [DONE] `crates/normalize-tools/src/adapters/ruff.rs:213` — same as eslint: `fix()` hardcodes `Warning` — apply same mapping as `run()` _(severity: medium)_
- [DONE] `crates/normalize-tools/src/tools.rs:178,227` — `find_js_tool`/`find_python_tool` check `node_modules/.bin` and `.venv/bin` relative to CWD, not `root` — add `root: &Path` parameter _(severity: medium)_
- [DONE] `crates/normalize-shadow/src/lib.rs:829-848,982-998,670-685` — `redo()`, `goto()`, `undo()` discard git add/commit errors with `let _ = ...`, returning success even when shadow history write fails — propagate as `ShadowError::Commit` _(severity: medium)_
- [DONE] `crates/normalize-tools/src/diagnostic.rs:7-18` vs `crates/normalize-output/src/diagnostics.rs:15-20` — parallel `DiagnosticSeverity` and `Severity` enums with identical variants; forces conversion at boundaries — unify on one enum _(severity: medium)_
- [DONE] `crates/normalize/src/service/tools.rs:86-120` — lint `--fix` rewrites files in-place with no `--dry-run` option; CLAUDE.md requires `--dry-run` for all mutating commands _(severity: medium)_
- [DONE] `crates/normalize/src/service/analyze.rs:304-315` — `security` method takes `_target: Option<String>` but ignores it; users who pass a target get whole-project analysis silently — plumb or remove the param _(severity: medium)_
- [DONE] `crates/normalize-languages/src/svelte.rs:73-80` — `get_visibility` uses text scan (`contains("export ")`) instead of child node inspection, unlike every other language; false positives on string literals — walk child nodes for export keyword _(severity: medium)_
- [DONE] `crates/normalize-languages/src/dart.rs:149` — `show`/`hide` imports marked `is_wildcard = true`; these are named imports, not wildcards; semantically wrong — set `is_wildcard = false`, populate `names` from `show` clause _(severity: medium)_
- [DONE] `crates/normalize-languages/src/ecmascript.rs:278,290-295` — namespace import (`import * as ns`) sets `is_wildcard: false` and pushes `"* as ns"` into `names`, producing invalid `{ * as ns }` in `format_import` — set `is_wildcard: true`, `alias = Some("ns")` _(severity: medium)_
- [DONE] `crates/normalize/src/service/analyze.rs:219-771` — none of the 16 `pub` methods in `AnalyzeService` have `Examples:` doc sections; all other services do — add examples _(severity: medium)_
- [DONE] `crates/normalize-facts/src/index.rs:2055-2057,2213-2215,2254-2255` — `resolve_all_imports/calls().await.ok()` silently discards resolution errors — log at warn level _(severity: medium)_
- [DONE] `crates/normalize-languages/src/registry.rs:394-463` — `validate_unused_kinds_audit` creates `used_kinds: HashSet<&str>` but never populates it; the check is permanently a no-op — populate or remove _(severity: medium)_
- [DONE] `crates/normalize-path-resolve/src/lib.rs:385-425` — `resolve()` silently strips the symbol component from colon-paths (`src/main.py:MyClass` → only file returned) without documenting this truncation _(severity: medium)_
- [DONE] `crates/normalize-path-resolve/src/lib.rs:38-47` — `find_like` returns `Option<Vec<(String, bool)>>` while `all_files` returns `Option<Vec<PathEntry>>`; same concept, different shapes — unify to return `Vec<PathEntry>` _(severity: medium)_
- [DONE] `crates/normalize-languages/src/external_packages.rs:84-94` — `Version::parse("1.2.3")` silently discards patch, returning `Version { major: 1, minor: 2 }` — document or reject 3-part versions _(severity: medium)_
- [DONE] `crates/normalize-graph/src/lib.rs:879` — `longest_path_from` memo cache subtlety: results cached with one `visited` set may return shorter paths when reached from a different root — document the limitation _(severity: medium)_

---

### error-surface (medium)

- [DONE] `crates/normalize-facts/src/index.rs:851,873,895,905,1113-1114` etc. — i64→usize casts without bounds check; negative DB values silently wrap to `usize::MAX` — use `u64::try_from(n).unwrap_or(0)` pattern _(severity: medium)_
- [DONE] `crates/normalize-facts/src/index.rs:394-413` — schema reset block uses `.ok()` on DELETE/ALTER TABLE; partial failure leaves DB in invalid state while version is written as valid — propagate errors _(severity: medium)_

---

### doc-coverage (low)

- [DONE] `crates/normalize-facts/src/index.rs:421-461` — SQL views `entry_points`, `external_deps`, `external_surface` created with no doc comments explaining their semantics _(severity: low)_
- [DONE] `crates/normalize-graph/src/lib.rs:838` — `find_longest_chains` hardcodes minimum chain length 4; undocumented _(severity: low)_
- [DONE] `crates/normalize-rules-config/src/lib.rs:77-99` — `RuleOverride::merge` doc doesn't explain practical impact of "cannot reset Vec to empty" _(severity: low)_
- [DONE] `crates/normalize-native-rules/src/lib.rs:22-37` — `id` naming convention not documented (slash-namespace vs hyphen) _(severity: low)_
- [DONE] `crates/normalize-path-resolve/src/lib.rs:23` — `PathEntry` doc claims returned by `find_like` but `find_like` still returns `(String, bool)` tuples — fix after unification _(severity: low)_
