# Pretty-output wiring audit

**Date:** 2026-06-28
**Scope:** every `#[cli]` command in the workspace that could lose `format_pretty()` output.
**Companion:** `diagnosis.md` (the `sessions stats` instance that triggered this audit).

---

## The defect class

Each `#[cli]` service struct holds its own `pretty: Cell<bool>` field, initialized to
`false` at construction. (Sub-services receive `Cell::new(pretty.get())` — a *copy* of
the root cell's value, not a shared cell; see `crates/normalize/src/service/mod.rs:100`
where the root is `Cell::new(false)`.) For a command's `format_pretty()` to ever run,
**both** of these must hold:

1. The method's `#[cli(display_with = "FN")]` routes to a fn that dispatches
   `if self.pretty.get() { r.format_pretty() } else { r.format_text() }`.
2. The method declares `pretty: bool` and `compact: bool` parameters **and** calls a
   resolver that sets `self.pretty` (`self.resolve_format(pretty, compact, &root)` in
   analyze/rank, `self.pretty.set(resolve_pretty(root, pretty, compact))` in sessions/
   rules).

If (1) holds but (2) does not, `format_pretty()` is **never** called — `--pretty` is
accepted (because the service declares `global = [pretty, compact]`) but silently
ignored, falling back to text. This is the **BROKEN** class, user-visible only when the
report type has a *real* (non-trivial) `format_pretty()` override that differs from
`format_text()`.

### Why `global = [...]` alone is insufficient — verified against server-less

`crates/server-less-macros/src/cli.rs` generates the leaf dispatch arm. Method
arguments come **only** from `regular_params` (the params the method actually declares):

- `generate_leaf_match_arm` builds `arg_extractions`/`arg_names` by iterating
  `regular_params` (cli.rs:1715). A param that is also a global flag is read from the
  propagated matches (`let #name: bool = sub_matches.get_flag(#name_str);`, cli.rs:1722–1732).
- `global = [...]` registers the flag on the **root** command and propagates it to
  sub-matches (cli.rs:1002–1115), but the macro does **not** synthesize a method
  argument for a global the method didn't declare. The value lands in `sub_matches`
  and is never read.
- The method call is `self.#method_name(#(#arg_names),*)` (cli.rs:1893) — strictly the
  declared params. The display fn is invoked separately as `self.#display_fn(&value)`
  (cli.rs:1924) with no global-flag context.

So a global flag's value reaches the body **iff the method declares it as a param**.
The diagnosis's claim ("each method needs the params") is correct.

---

## Part 1 — Command-by-command audit

Verdicts: **WORKING** (real pretty, dispatching display fn, flag wired) ·
**BROKEN** (real pretty, dispatching display fn, flag *not* wired) ·
**N/A** (no real pretty to lose, or display fn never dispatches to pretty).

### BROKEN — silently-no-op pretty (8 commands)

| command | file:line | report (real format_pretty) | why broken |
|---|---|---|---|
| `sessions stats` | `service/sessions.rs:242` | `SessionAnalysisReport` (`normalize-session-analysis/src/lib.rs:901`) | no `pretty`/`compact` params, no `self.pretty.set` |
| `sessions subagents` | `service/sessions.rs:568` | `SubagentsReport` (`commands/sessions/subagents.rs:57`) | params are only session/format/project/root |
| `analyze architecture` | `service/analyze.rs:139` | `ArchitectureReport` (`commands/analyze/architecture.rs:143`) | sig ends at `limit`; body never calls `resolve_format` |
| `analyze cross_repo_health` | `service/analyze.rs:419` | `CrossRepoHealthReport` (`commands/analyze/cross_repo_health.rs:79`) | only repos_dir/repos_depth params; no resolve |
| `rank files` | `service/rank.rs:312` | `FileLengthReport` (`commands/analyze/files.rs:112`) | params limit/root/exclude/diff; no resolve |
| `rank size` | `service/rank.rs:357` | `SizeReport` (`commands/analyze/size.rs:41`) | params root/exclude; no resolve |
| `rank ceremony` | `service/rank.rs:381` | `CeremonyReport` (`commands/analyze/ceremony.rs:191`) | params root/limit/diff; no resolve |
| `rank contributors` | `service/rank.rs:1595` | `ContributorsReport` (`commands/analyze/contributors.rs:144`) | params repos_dir/repos_depth; no resolve |

All eight services declare `global = [pretty, compact]` (sessions.rs:66, analyze.rs:113,
rank.rs:195), so `--pretty` is advertised in `--help` and accepted — it just does
nothing. These are live user-visible regressions.

### WORKING (~46 commands)

- **sessions:** `list`, `show`, `analyze`, `messages`, `patterns`, `parallelization`,
  `heatmap`, `cost` — all declare `pretty`/`compact` and call
  `resolve_pretty(...)` (e.g. sessions.rs:122, 173, 218, 490, 635, 702, 780, 856).
- **analyze:** `health`, `all`, `coupling_clusters`, `summary`, `skeleton_diff`.
- **rank:** `complexity`, `hotspots`, `test_ratio`, `length`, `test_gaps`, `budget`,
  `density`, `uniqueness`, `surface`, `depth_map`, `layering`, `module_health`,
  `call_complexity`, `duplicates`, `fragments`.
- **normalize (mod/view/trend/context):** `grep`, `ci`, `view`, `chunk`, `list`,
  `blame`, `trend multi|complexity|length|density|test-ratio`, `context` (default) and
  `context show`.
- **normalize-rules:** `list`, `run`, `test`, `test-fixtures` (per-method
  `resolve_format`, `display_output`/`display_run` dispatch).

### N/A (no real pretty, or display fn never dispatches)

- **Report has no real pretty** (trait default or `format_pretty` == `format_text`):
  `analyze security|docs|activity|repo_coupling|liveness|effects|exceptions`,
  `rank coupling|ownership|imports|duplicate_types`, `aliases`, `init`, `update`,
  `docs`, `sync`, `view history|dependents|graph|import-path`, all `config`,
  all `grammars`, `sessions ngrams|plans|mark|unmark`, all `normalize-rules`
  enable/disable/show/tags/add/update/remove/setup/validate/compile, all of
  `normalize-facts`, `normalize-filter`, `normalize-syntax-rules`,
  `normalize-knowledge-graph`, `normalize-cfg`.
- **Display fn never dispatches to pretty** (calls `format_text()`/custom text
  unconditionally — `self.pretty` is irrelevant): `translate`, `view referenced-by`,
  `view references`, `view trace`, all `syntax`, all `package`, all `tools lint`,
  all `daemon`, all `generate`, all `guide`, all `structure`/facts, all
  `normalize-ratchet`, all `normalize-budget`.

---

## Part 1b — Adjacent output-machinery defects found while auditing

These are not the strict BROKEN class (the display fn never dispatches, so no method
wiring would help), but they have the **same user-visible outcome**: a real
`format_pretty()` was written and can never be reached.

1. **`edit` refactor commands — unreachable `format_pretty` (6 commands).**
   `MoveReport`, `IntroduceVariableReport`, `InlineVariableReport`,
   `AddParameterReport`, `InlineFunctionReport`, `ExtractFunctionReport` each define a
   substantial `format_pretty` (`service/edit.rs:1766/1898/2076/2253/2420/2564`), but
   their display fns (`display_move` etc., edit.rs ~936–958) unconditionally call
   `report.format_text()`. The pretty renderings are dead code.

2. **`syntax node-types` — unreachable `format_pretty`.** `NodeTypesReport` has a real
   `format_pretty` (`commands/syntax/node_types.rs:58`) but `display_node_types` calls
   `format_text` directly.

3. **`grammars` — latent BROKEN, currently masked.** `GrammarService.display_output`
   *does* dispatch on `self.pretty.get()`, but no grammar method declares
   `pretty`/`compact` or resolves, and `self.pretty` is a construction-time copy that
   stays `false`. Only saved because the grammar reports have no real `format_pretty`.
   The moment one gains a pretty override this becomes a live `sessions stats`-style bug.

4. **`normalize-ratchet` / `normalize-budget` — dead `--pretty` flag.** Both have the
   *full* plumbing (`pretty: Cell<bool>`, `resolve_format` called in every method,
   `global = [pretty, compact]` advertised) but every `display_*` fn hardcodes
   `r.format_text()` and no report implements `format_pretty`. `--pretty` is accepted,
   threaded into the Cell, and never read. Harmless today, but an advertised no-op.

---

## Part 2 — Structural assessment (root cause)

**Yes — this should be fixed structurally in server-less.** The manual
"declare `pretty`/`compact` params on every method + call `resolve_*` in every body"
pattern is the footgun: it's redundant (the same two params + one call copy-pasted
across ~50 methods), and omitting it fails *silently* (no compile error, `--pretty`
still parses). Per this repo's CLAUDE.md, a proc-macro pattern that breaks this often is
a server-less UX bug to fix there, not to document around. The bug recurred (stats,
subagents, architecture, cross_repo_health, 4× rank) precisely because the wiring is
opt-in-per-method rather than automatic.

### Why it can't be fixed by `global` alone today

The macro delivers a global flag's value to the body **only** when the method declares a
matching param (cli.rs:1715–1814, call at 1893). The display fn (cli.rs:1924) gets no
global context at all — it relies on the service mutating `self.pretty` from inside the
method body, which the macro neither knows about nor performs.

### Feasibility: high. Sketch of the fix.

Add a hook so the macro delivers declared global-flag values to the service
*automatically*, decoupled from each method's signature:

```rust
// server-less
pub trait CliGlobals {
    /// Called once per dispatch, before the method runs, for each declared
    /// `global = [...]` bool flag. Default no-op.
    fn set_global_flag(&self, _name: &str, _value: bool) {}
}
```

In `generate_leaf_match_arm`, for each entry of `global_flags` that is a bool, emit
(before `gen_call`):

```rust
<S as ::server_less::CliGlobals>::set_global_flag(self, "pretty",   sub_matches.get_flag("pretty"));
<S as ::server_less::CliGlobals>::set_global_flag(self, "compact",  sub_matches.get_flag("compact"));
```

Then normalize implements `CliGlobals` **once per service**:

```rust
impl CliGlobals for AnalyzeService {
    fn set_global_flag(&self, name: &str, value: bool) {
        match name {
            "pretty"  => self.pretty_raw.set(value),
            "compact" => self.compact_raw.set(value),
            _ => {}
        }
    }
}
```

and `display_output` resolves lazily (`!compact && (pretty || config.pretty.enabled())`,
defaulting the config root to cwd). Methods drop their `pretty`/`compact` params and
their `resolve_format` calls entirely. The footgun is gone: every command with the
globals declared gets them wired by construction, and there is nothing per-method left
to forget.

Notes / open questions for the server-less change:
- **Config/TTY resolution moves out of the method.** Today `resolve_pretty(root, ...)`
  reads `NormalizeConfig::load(root)` using the method's `root` param. With auto-wiring
  the display fn no longer has `root`; resolution must default the config root to cwd
  (or normalize stores raw flags and resolves against the command's already-known root
  via a different channel). This is a normalize-side concern, not server-less's.
- **Blanket default impl** keeps it backward-compatible: services that don't implement
  `CliGlobals` (the text-only feature crates) get the no-op and are unaffected.
- **Alternative considered:** pass a `&GlobalFlags` map into `display_with` and drop the
  `Cell` entirely. Cleaner in principle (no interior mutability), but changes the
  `display_with` fn signature for every existing consumer — a larger migration. The
  trait-hook approach is the minimal change that removes the footgun. Recommend the
  trait hook; revisit the signature change only if the `Cell` causes further problems.

This belongs in server-less's TODO; it is the correct root-cause fix. The immediate
in-repo fix is still to wire the 8 BROKEN methods (and reach the 7 unreachable-pretty
display fns) so users get correct output before the server-less change lands.
