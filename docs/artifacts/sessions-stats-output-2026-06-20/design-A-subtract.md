# Design A — SUBTRACT: collapse pretty/text into one rendering primitive

**Date:** 2026-06-28
**Frame:** MINIMIZE / SUBTRACT — fewest moving parts, fewest concepts. Find the single
primitive that makes "pretty vs text" stop being a special case, so there is nothing
left to forget.
**Companions:** `pretty-wiring-audit.md` (mechanism + 16 broken commands), `diagnosis.md`
(the `sessions stats` instance).

---

## 0. What we are subtracting

Today, getting human output from a `#[cli]` command requires **seven** distinct concepts
to line up, and three independent ways to get it wrong (the (a)/(b)/(c) defect classes):

| # | concept | where | what breaks if omitted |
|---|---|---|---|
| 1 | `format_text()` | report (`OutputFormatter`) | — (always present) |
| 2 | `format_pretty()` | report (`OutputFormatter`) | falls back to text (default) |
| 3 | `display_with` fn | service | (c) dead dispatch if it ignores the flag |
| 4 | `self.pretty: Cell<bool>` | service struct | (a) stays `false` forever |
| 5 | `resolve_pretty(root, p, c)` call | method body | (a) Cell never set |
| 6 | `pretty: bool, compact: bool` params | method signature | (a) flag value never reaches body |
| 7 | `global = [pretty, compact]` | `#[cli(...)]` attr | (b) advertised but, w/o 2, no-op |

The defects are not bugs in any one of these — they are the *seams between them*. The
flag value is parsed by the macro (7), must be re-declared as a param to enter the body
(6), resolved against config there (5), stashed in interior-mutable state (4), and read
back out by a hand-written bridge (3) to pick between two render methods (1/2). Five hops,
each hand-wired per method, each failing silently.

**The asymmetry that explains everything:** the macro *already* owns machine-format
selection end-to-end. For `--json`/`--jsonl`/`--jq` it reads the flags from `sub_matches`,
resolves the format, and renders the value itself via `cli_format_output(serde_value, …)`
(cli.rs:1950–1959). There is no `Cell`, no per-method param, no bridge fn, nothing to
forget — and correspondingly **zero** json/jsonl/jq defects in the audit. The human
formats (text/pretty) are the *only* output axis the macro delegates back to hand-written
service state, and they are the *only* axis with a defect class.

The subtraction is therefore obvious: **make human rendering work exactly like machine
rendering — one value, one rendering call the macro always drives, mode resolved by the
macro from the flags.** Concepts 3, 4, 5, 6 disappear entirely. Concepts 1+2 collapse into
one. Concept 7 dissolves (see §3b).

---

## 1. The primitive

> **There are exactly two rendering axes, both owned by the macro: machine output is
> derived generically from `Serialize` (unchanged), and human output comes from a single
> trait method `render(&self, mode) -> String` that the macro always calls with a
> macro-resolved `RenderMode`.**

That single trait method is the whole primitive. It replaces `format_text`,
`format_pretty`, `display_with`, `self.pretty`, and the in-body `resolve_pretty` call with
one thing the macro drives unconditionally.

### 1a. The trait (server-less, replaces `OutputFormatter`'s two methods)

```rust
// server-less
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RenderMode { Plain, Pretty }

/// A typed value that can render itself as human-facing CLI text.
/// Machine formats (json/jsonl/jq) are NOT here — they come from `Serialize`.
pub trait CliRender {
    fn render(&self, mode: RenderMode) -> String;
}
```

A report with no rich form ignores the argument; a report with a rich form branches on it
**in one place**:

```rust
impl CliRender for SessionAnalysisReport {
    fn render(&self, mode: RenderMode) -> String {
        match mode {
            RenderMode::Pretty => self.write_pretty(),   // the bar charts / ━━━ headers
            RenderMode::Plain  => self.write_text(),      // the markdown pipe tables
        }
    }
}
```

There is no longer a "default that silently falls back": *falling back is just the `Plain`
arm of a `match` you wrote yourself*. You cannot accidentally route around it because the
macro never calls anything else.

### 1b. Macro responsibility (server-less, in `gen_value_display`)

The text branch of `gen_value_display` (cli.rs:1922–1947) changes from "call the
hand-written `display_with` fn" to "resolve the mode and call `render`":

```rust
// before (today): delegates to a service fn that reads self.pretty
let __display = self.#display_fn(&value);
println!("{}", __display);

// after: macro resolves the mode and drives the single render method
let __mode = self.__resolve_render_mode(
    sub_matches.get_flag("pretty"),
    sub_matches.get_flag("compact"),
    __render_root,            // see §4
);
println!("{}", ::server_less::CliRender::render(&value, __mode));
```

The json/jsonl/jq branch (cli.rs:1950–1959) is **untouched** — machine formats stay
generic over `Serialize`. The only change is that the human branch is now symmetric with
it: macro-resolved, macro-driven, nothing delegated to mutable service state.

### 1c. Mode resolution (impl-level hook, default-safe)

`__resolve_render_mode` is generated. By default server-less supplies a TTY-only resolver
(no config, no root needed); a consumer that wants config-driven pretty supplies one fn
**once per `#[cli] impl` block** via a new impl-level attribute, mirroring how
`display_with` works today but at the impl level (once) instead of per method:

```rust
#[cli(
    name = "sessions",
    render_mode = "resolve_render_mode",   // optional; omit -> built-in TTY resolver
)]
impl SessionsService { … }

impl SessionsService {
    // written ONCE per service, not per method
    fn resolve_render_mode(&self, pretty: bool, compact: bool, root: Option<&Path>) -> RenderMode {
        let root = root.unwrap_or_else(|| Path::new("."));
        if !compact && (pretty || NormalizeConfig::load(root).pretty.enabled()) {
            RenderMode::Pretty
        } else {
            RenderMode::Plain
        }
    }
}
```

Built-in default when `render_mode` is omitted:

```rust
fn default_render_mode(pretty: bool, compact: bool, _root: Option<&Path>) -> RenderMode {
    if compact { RenderMode::Plain }
    else if pretty || std::io::stdout().is_terminal() { RenderMode::Pretty }
    else { RenderMode::Plain }
}
```

This default is the linchpin of "impossible to silently no-op": *even a consumer who wires
nothing gets correct flag + TTY behavior.* The config nuance is the only thing you can
forget, and forgetting it degrades gracefully (TTY still works) rather than silently
killing `--pretty`.

### What the command author writes — the whole surface, after

- Report: `impl CliRender { fn render(&self, mode) { match mode { … } } }` — one method.
- Service: one `resolve_render_mode` fn + `render_mode = "…"` on the impl. Once. Not per command.
- Per method: **nothing.** No `pretty`/`compact` params, no `resolve_*` call, no
  `display_with`, no `Cell`.

---

## 2. Before / after, concrete

### 2a. BROKEN command — `sessions stats`

**Before** (`service/sessions.rs:242`): method declares 14 params, none of them
`pretty`/`compact`; body never calls `resolve_pretty`; `self.pretty` stays `false`;
`display_output` reads it and always picks `format_text()`. `--pretty` parses (because
`global = [pretty, compact]` at sessions.rs:66) and is silently ignored.

```rust
#[cli(display_with = "display_output")]
pub fn stats(&self, …14 params…, by_repo: bool)            // no pretty/compact
    -> Result<SessionAnalysisReport, String> {
    // …no self.pretty.set(resolve_pretty(...))…
    build_stats_data(…)
}
```

**After:** the signature loses nothing it had and gains nothing — it is *already correct*.
The render path is the macro's job now:

```rust
#[cli]                                                     // no display_with
pub fn stats(&self, …14 params…, by_repo: bool)
    -> Result<SessionAnalysisReport, String> {
    build_stats_data(…)                                    // unchanged body
}
```

`SessionAnalysisReport` gains the merged `render`:

```rust
impl CliRender for SessionAnalysisReport {
    fn render(&self, mode: RenderMode) -> String {
        match mode { RenderMode::Pretty => self.write_pretty(), RenderMode::Plain => self.write_text() }
    }
}
```

The bug is gone because *the method never had a role in rendering* — the macro resolves the
mode (via `SessionsService::resolve_render_mode`, which folds in config rooted at the
method's `root`, see §4) and calls `render`. There is no longer any per-method step to
omit. (The `by_repo` / `group_by` `process::exit` paths that print directly are addressed
in §5.)

### 2b. WORKING command — `sessions list`

**Before** (`service/sessions.rs:89`): correct *only because* the author remembered all of
it — declared `pretty: bool, compact: bool` (115–116), called
`self.pretty.set(resolve_pretty(resolved_root, pretty, compact))` (122–123), and the report
implements `format_pretty`. The same five hops as `stats`, just not forgotten.

```rust
#[cli(display_with = "display_output")]
pub fn list(&self, …, pretty: bool, compact: bool) -> Result<SessionListReport, String> {
    let resolved_root = root_path.unwrap_or(Path::new("."));
    self.pretty.set(resolve_pretty(resolved_root, pretty, compact));   // the boilerplate
    …
}
```

**After:** the two params and the `resolve_pretty` line are **deleted**. The method shrinks
to its actual job (building the list). It now looks *identical in shape* to `stats` — which
is the point: the working and broken commands converge on one shape, so there is no
"remembered vs forgot" axis left.

```rust
#[cli]
pub fn list(&self, …no pretty/compact…) -> Result<SessionListReport, String> { … }
```

`SessionListReport`'s two `OutputFormatter` methods merge into one `CliRender::render`.

---

## 3. How each defect becomes impossible

### (a) SILENT NO-OP — *impossible by construction*

Mechanism: **there is no per-method wiring to omit.** The fields it depended on are gone —
no `self.pretty` Cell to leave unset (deleted), no `pretty`/`compact` params to forget to
declare (deleted), no `resolve_pretty` call to forget (moved to one generated call site in
the macro). The macro *unconditionally* emits
`CliRender::render(&value, self.__resolve_render_mode(...))` in the text branch, the same
way it unconditionally emits `cli_format_output(...)` in the json branch. A command author
literally cannot write the code that produced this defect, because the code that produced
it (the manual hop chain) no longer exists in the program.

### (b) ADVERTISED NO-OP — *dissolved (the concept is deleted)*

This defect was "service declares `global = [pretty, compact]` but no report has a real
`format_pretty`." It existed only because advertising the flag (concept 7) was a *separate,
independent act* from implementing pretty (concept 2). In the new design there is no
separate act: `--pretty`/`--compact` are registered uniformly by server-less for every
`CliRender`-returning command (they are intrinsic to the human-render axis, not an opt-in
per service). Every command accepts `--pretty` and renders the best form it has; a report
whose `render` returns the same string for both arms is *honestly* "this command has no
distinct pretty form," not a wiring mismatch. There is no longer a state where the flag is
"advertised but unwired," because advertising and wiring are the same fact.

Residual (the one thing this frame does not compile-catch): a report that *intends* a
distinct pretty form but whose `render` happens to ignore `mode`. That is caught in CI, not
the type system — see §3-CI.

### (c) DEAD DISPATCH — *impossible by construction*

This defect was "a `display_with` fn calls `format_text()` unconditionally, so a written
`format_pretty()` is dead." Mechanism of prevention: **there is no `display_with` fn to
write wrong.** The only renderer is `CliRender::render(mode)`, and the *macro* — not the
author — calls it with the resolved mode. The mode-branch lives *inside* `render`, the same
method that holds both arms, so "the dispatcher ignores the flag" is not expressible:
there is no separate dispatcher, and the only way to "ignore the mode" is to write a
`match` whose arms are identical, which is the (b) residual, not a bypass of dead code.
(`edit`'s six refactor reports and `syntax node-types`, listed as adjacent (c) cases in the
audit, are fixed by the same collapse: their `display_move`-style fns are deleted and their
`format_pretty` bodies move into the `Pretty` arm of `render`.)

### §3-CI — the exhaustiveness backstop for the (b) residual

The existing `assert_output_formatter::<T>()` compile test in `output.rs` (a hand-listed
registry of every report type) is repurposed and strengthened. Two CI mechanisms:

1. **Trait-bound exhaustiveness (compile-time):** change the registry asserts from
   `assert_output_formatter::<T>()` to `assert_cli_render::<T>()`. Any report wired into a
   `#[cli]` method that does not implement `CliRender` fails to compile (same guarantee as
   today, retargeted to the one trait).
2. **Pretty-distinctness (test-time, opt-in):** for the subset of reports that declare a
   real pretty form (tagged, e.g. `#[derive(CliRender)] #[render(has_pretty)]` or listed in
   a `PRETTY_REPORTS` set), a unit test asserts `r.render(Pretty) != r.render(Plain)` on a
   fixture instance. This is the *only* check the type system can't give for free, and it
   converts the audit's manual "real format_pretty?" column into an automated gate.

---

## 4. Root-aware + TTY resolution

The wrinkle: `resolve_pretty(root, …)` consults `NormalizeConfig::load(root).pretty` where
`root` is the command's *target* dir (e.g. `--root /elsewhere`), not cwd. Naive
auto-wiring that loads config from cwd is wrong when `--root` points elsewhere. The Cell
exists today *precisely* to carry this per-invocation, root-derived decision from the
method body (which has `root`) out to the rendering bridge (which doesn't).

The new design keeps correctness without the Cell by observing: **the macro has already
extracted every method parameter — including `root` — into local bindings before it
renders** (`arg_extractions` run before `#output`, cli.rs:2075→2078). So the resolved root
is in scope at the render call site; it just needs a name the macro can refer to. We give
it one with a param marker:

```rust
pub fn stats(
    &self,
    …,
    #[param(short = 'r', render_root, help = "Root directory")] root: Option<String>,
    …
) -> Result<SessionAnalysisReport, String> { … }
```

The macro, when a param carries `render_root`, threads that binding into the resolver call:

```rust
let __render_root: Option<&Path> = root.as_deref().map(Path::new);   // from the marked param
let __mode = self.resolve_render_mode(
    sub_matches.get_flag("pretty"), sub_matches.get_flag("compact"), __render_root,
);
```

`resolve_render_mode` then does exactly what `resolve_pretty` does today —
`NormalizeConfig::load(root).pretty.enabled()` — but in **one** consumer-written fn per
service instead of inlined into ~50 method bodies.

Flow summary:

```
--pretty/--compact  ─┐                              (raw flags, from sub_matches)
--root <path> param ─┼─► macro (already-extracted) ─► self.resolve_render_mode(p, c, root)
TTY (isterminal)    ─┘                                   │  folds in NormalizeConfig::load(root)
                                                         ▼
                                              RenderMode {Plain|Pretty}
                                                         │
                                            macro: value.render(mode)
```

Degradation is graceful and *never silent*: if a command omits `render_root`, the resolver
gets `None` → defaults config root to cwd. That loses only the "project at a different
root sets `pretty` in its config" nuance; the `--pretty` flag and TTY detection still work
fully. Contrast with today, where omitting the wiring kills `--pretty` *entirely*. The
worst failure mode moves from "flag does nothing" to "flag works, project config override
ignored for non-cwd roots" — a strict improvement, and itself lintable (a native rule:
"`#[cli]` method has a `root` param but no `render_root` marker").

Why not just blanket-resolve in the macro? Because config loading is normalize-specific;
server-less must stay generic. The `render_mode` impl-hook is the seam that keeps
server-less ignorant of `NormalizeConfig` while still owning the *dispatch*. server-less
contributes the default (flags + TTY); normalize contributes config; the macro composes
them.

---

## 5. Migration plan

Scope: ~46 working + ~16 broken + ~7 adjacent dead-dispatch (`edit` ×6, `syntax
node-types`) = the entire human-render surface, ~80 report types and ~12 services.

**Phase 0 — server-less (one PR in `/home/me/git/rhizone/server-less`):**
1. Add `RenderMode` enum + `CliRender` trait + blanket-free `default_render_mode`.
2. Parse impl-level `render_mode = "FN"` (alongside `global`, cli.rs:214–282) and the
   `render_root` param marker (alongside `positional`/`short`, in `generate_arg`/param parse).
3. Rewrite the text branch of `gen_value_display` (cli.rs:1922–1947) to resolve mode and
   call `CliRender::render`. **Keep** the `display_with` and `Display` branches intact for
   backward compat — this is purely additive for other consumers.
4. Register `--pretty`/`--compact` as built-in human-render globals for any command whose
   return type is `CliRender` (so consumers stop hand-declaring them in `global = [...]`).

**Phase 1 — normalize, mechanical, per service (can parallelize across services):**
5. Reports: merge `format_text`+`format_pretty` → `CliRender::render(mode){ match … }`.
   Where `format_pretty` was the trait default (no real pretty), `render` is just
   `_ => self.format_text_body()` — both arms identical. ~80 types, purely mechanical.
6. Services: delete the `pretty: Cell<bool>` field and its `new(&pretty)` threading; delete
   every `display_with`/`display_output`/`display_analyze`/`display_move` fn; delete
   `pretty`/`compact` params and `resolve_pretty` calls from every method; add one
   `resolve_render_mode` fn + `#[cli(render_mode = "resolve_render_mode")]`; mark the
   `root` param `render_root`.
7. Refactor the **print-and-exit** methods that bypass the framework: `sessions stats`
   `by_repo` and `group_by` branches currently `println!(self.display_output(&r))` then
   `process::exit`. These must return their report and let the macro render. `by_repo`
   already builds a `SessionAnalysisReport` → just `return` it. `group_by` prints *N* groups
   → needs a `GroupedStatsReport` wrapper (a `Vec` of groups) whose `render` joins them.
   This is the only place the subtraction touches data shape, and it fixes a pre-existing
   wart (a command that `process::exit`s out of the service layer is already an anomaly).
8. Update `output.rs`: `assert_output_formatter` → `assert_cli_render`; add the
   `has_pretty` distinctness test set (§3-CI).
9. Delete `resolve_pretty` from `service/mod.rs` and the per-sub-service `pretty` cells.

**Phase 2 — cleanup / lint:**
10. Add a native rule: `#[cli]` method with a `root` param lacking `render_root` (catches
    the §4 degradation).
11. Update `docs/cli/`, `README.md`, `LLMS.md`, `docs/cli-design.md`, CHANGELOG.

**Cost / risk / blast radius:**
- **server-less blast radius (Phase 0):** additive. New trait + enum + two attribute knobs +
  one rewritten branch. Existing `display_with` and `Display` paths are preserved, so **other
  server-less consumers are unaffected** unless they opt in. Risk: low; the rewritten branch
  is exercised by normalize's whole test suite immediately.
- **normalize blast radius:** large but shallow and mechanical (~80 reports, ~12 services,
  ~50 method signatures). No algorithm changes; the report *bodies* (`write_pretty`,
  `write_text`) are reused verbatim inside `render`. The risky 10% is step 7 (the
  print-and-exit refactor and the `GroupedStatsReport` shape).
- **Sequencing:** Phase 0 must land and publish (server-less is published; normalize depends
  on a version, no path deps) before Phase 1 — a real cross-repo coordination cost.
- **Single biggest risk:** the `group_by` print-and-exit path. It is the one spot where the
  current code escapes the "return typed data, macro renders" contract, and the subtraction
  *requires* pulling it back in (it can't render a multi-group result through a single
  `render` call without a wrapper report). Everything else is find-and-replace; this needs a
  genuine (small) data-model decision.

---

## 6. Honest trade-offs

**Where this frame is strong:**
- Deletes 4 of 7 concepts outright (`display_with`, `Cell`, in-body resolve, per-method
  params) and merges 2 more into 1. The remaining surface is one trait method + one
  per-service resolver.
- (a) and (c) become *impossible by construction* — not lint-caught, not test-caught, but
  unwritable, because the code paths that expressed them are gone.
- Restores symmetry with the already-correct machine-format path: human and machine output
  are now both "one value, macro-resolved format, macro-driven render." The defect class
  existed *only* in the asymmetric axis; the fix is to remove the asymmetry.
- Mode flows as a *value parameter*, not interior-mutable service state. This kills the
  `Cell` and aligns with the repo's own "configuration flows in via constructors/params,
  not out via globals at call sites" rule — the per-invocation render mode was exactly such
  a smuggled-in global. It also makes services trivially `Send`/concurrent (an LSP/daemon
  embedding two roots can't race on a shared `pretty` cell, because there isn't one).

**Where this frame is thin:**
- **(b) is dissolved, not compile-caught.** A report that *intends* distinct pretty output
  but writes a `render` whose arms coincide is indistinguishable, to the type system, from
  one that legitimately has no pretty form. The §3-CI distinctness test is the backstop, and
  it requires fixtures + a maintained `has_pretty` tag — the one piece of per-report
  bookkeeping that survives. (This is irreducible: "did the author *mean* to differ" is
  semantic, not structural.)
- **Two render methods → one with a `match` is marginally less ergonomic.** Reports that
  shared most of their layout across text/pretty now branch inside one method. In practice
  they already did (both `format_*` called shared helpers), so it's close to net-neutral,
  but it's a real readability cost for the few reports with large divergent bodies.
- **`render_root` is still a per-command marker.** Forgetting it doesn't reintroduce the
  silent no-op (TTY/flag still work) but does ignore project config for non-cwd roots. We
  mitigate with a lint, but it's the one residual "remember to annotate" — a much weaker
  footgun than the original five-hop chain, yet not zero.
- **Cross-repo sequencing.** The fix lives in server-less (correctly — it's a macro UX bug
  per CLAUDE.md), so it can't land atomically with normalize. Publish-then-bump is a real
  coordination tax that an in-normalize-only patch would avoid.
- **Materialization.** `render -> String` forces whole-output materialization (no streaming
  writer). The current code already does this, so no regression — but the subtraction passes
  up the chance to introduce a `Write`-based renderer.
```