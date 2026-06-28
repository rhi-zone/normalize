# Judge — completeness bake-off: which defects survive each design

**Date:** 2026-06-28
**Role:** adversarial judge. For each of the four designs (A subtract, B type-property,
C invert, D build-guard) I tried to construct a concrete escape — a way a careless or
adversarial developer still ships broken/no-op output despite the design being in place.
**Inputs read & grounded:** the four design docs, `pretty-wiring-audit.md`, and the live
source (`crates/normalize/src/output.rs`, `service/mod.rs::resolve_pretty`,
`service/sessions.rs`). Confirmed: `resolve_pretty(root,p,c) = !compact && (pretty ||
NormalizeConfig::load(root).pretty.enabled())` — TTY detection lives *inside*
`config.pretty.enabled()`, not in `resolve_pretty` itself; the `pretty` Cell is shared
by-reference to most sub-services but **copied** to `budget`/`ratchet`
(`Cell::new(pretty.get())`, mod.rs) — the latent bug the audit flagged; and
`assert_output_formatter` is a hand-maintained ~150-entry list that already shows drift.

The defect classes: **(a)** silent no-op (real `format_pretty` never dispatched),
**(b)** advertised no-op (`--pretty` advertised, no distinct output), **(c)** dead dispatch
(`display_with` calls `format_text` unconditionally), **(d)** root/TTY mis-resolution.

---

## The two universal residuals (no design closes these via types)

Before scoring designs, two escapes survive **all four** because they are semantic /
out-of-band, not structural:

### U1 — "pretty bytes == text bytes" (the (b)-by-accident residual)
A report whose rich form is byte-identical to its text form. The type system cannot know
whether two `String`-returning bodies differ.

```rust
impl PrettyFormat for FooReport {                 // (B); analogous arms in A/C, marker in D
    fn format_pretty(&self) -> String {
        self.format_text()                        // TODO stub, or copy-paste, or refactor drift
    }
}
```

This advertises `--pretty`, dispatches correctly, and produces no visible difference. The
*only* thing that catches it is a **behavioural / distinctness test** that renders both ways
and asserts `strip_ansi(pretty) != strip_ansi(text)` on a populated fixture. A and D ship
such a test (A §3-CI distinctness; D Layer 3). B and C as written do **not** (B offers
nothing; C offers it only as an optional lint). So U1 survives A and D *weakly* (deletable
test) and survives B and C *fully*.

### U2 — body bypasses the render path entirely
Any method that prints inside its body and exits, or returns `()`, never reaches the
macro's uniform render call. This is exactly the existing `sessions stats` `by_repo`/
`group_by` shape (`println!(self.display_output(&r)); process::exit(0)`).

```rust
pub fn stats(&self, /*…*/) -> Result<SessionAnalysisReport, String> {
    if by_repo {
        println!("{}", build_repo_table(&data));  // never returns through the macro
        std::process::exit(0);                     // all of a/b/c/d reappear here
    }
    Ok(build_stats_data(/*…*/))
}
```

Every type-based design (A/B/C) is powerless against this — they govern the *return-value*
render path; a developer who doesn't return a value isn't on it. Only a behavioural CLI test
(D Layer 3, run against the actual binary) can observe it. U2 survives A/B/C fully and D
weakly. **This is the strongest single argument for keeping a behavioural test no matter
which redesign wins.**

---

## Design A — SUBTRACT (single `CliRender::render(mode)`)

**Irreducible per-command author action:** `impl CliRender` (the macro *requires* it →
**compile-LOUD** if missing) + write distinct `match` arms (semantic) + mark the target
param `#[param(render_root)]` (**silent-degraded** if forgotten). Getting pretty to *work
at all* is automatic — the render method is compile-required and the macro always drives it;
there is no separate optional impl to forget. The only silent omission is second-order
(config-at-non-cwd root).

**Surviving defects:**
- **S-A1 (= U1).** `render`'s two arms coincide → `--pretty` (advertised on *every*
  `CliRender` command) is a no-op. Caught only by the opt-in §3-CI distinctness test, which
  needs the `has_pretty` tag set and is deletable.
- **S-A2 (d).** Forget `#[param(render_root)]` on a positional-target command (`view <path>`,
  `analyze <path>` — target is positional, not named `root`): `resolve_render_mode` gets
  `None` → config resolves against **cwd**, not `<path>`. `--pretty`/TTY still work, so it's
  silent-degraded. Mitigating native lint is itself deletable.
- **S-A3 (= U2).** print-and-exit / `()` body bypasses render.
- **S-A4 adversarial.** The consumer-written `resolve_render_mode` can be written to always
  return `Plain` — neuters pretty service-wide, no guard sees it.
- **Mass-(b) caveat.** A auto-advertises `--pretty` on *every* command (advertising and
  wiring "are the same fact"). Under a strict reading of (b) ("don't advertise a flag that
  does nothing"), every genuinely text-only report (~80 of them) now advertises a no-op
  `--pretty`. A reframes this as honest ("every command renders its best form"); but the
  literal symptom of (b) is present at scale. Counts as survivor only under the strict
  definition.

**Survivor count: 4** (S-A1..S-A4), +1 definitional (mass-advertise). **Worst:** S-A3/U2.
**(a) and (c): impossible by construction** (no separate dispatcher, no per-method wiring).

---

## Design B — TYPE PROPERTY (`PrettyFormat` sub-trait)

**Irreducible per-command author action:** for a command that *wants* pretty, write a
**separate `impl PrettyFormat` block** — **silent** if forgotten (no flag, no dispatch, no
error; indistinguishable from an intentional text-only command). Plus root marker for
non-`root`-named targets (silent-degraded).

**Surviving defects:**
- **S-B1 (= U1). WORST.** `impl PrettyFormat` with `format_pretty(){ self.format_text() }`
  → advertises + dispatches + identical output. B's core proposes **no** distinctness test,
  so nothing catches it. This is strictly worse than A here: A at least pairs the residual
  with §3-CI.
- **S-B2 (want-pretty-forgot-impl).** New command, author wants pretty, forgets the extra
  `impl PrettyFormat`:
  ```rust
  impl OutputFormatter for NewReport { fn format_text(&self)->String {/*…*/} }
  // no impl PrettyFormat  → --pretty silently absent, looks intentional
  ```
  Silent. This is the one place B is *weaker* than A: A's render method is compile-required
  (you can't have a report with no render path), whereas B's pretty capability is a
  *separate, omittable* impl block — more forgettable.
- **S-B3 (d).** Target param not named `root` and no `#[param(pretty_root)]` → config at cwd.
  Silent-degraded.
- **S-B4 (= U2).** print-and-exit / unit body.
- **S-B5 adversarial.** `PrettyPolicy::resolve_pretty` impl written always-false → neuters
  pretty. Consumer code, no guard.

**Survivor count: 5** (S-B1..S-B5). **Worst:** S-B1.
**(a) and (c): impossible by construction.** **Unique strength: (b)-as-false-advertising is
genuinely ELIMINATED** — `--pretty` is registered *iff* `<PrettyProbe<RetTy>>::HAS_PRETTY`,
so text-only commands never advertise it. B is the only design where advertising tracks real
capability, so it's the only one robust under the *strict* reading of (b). **Fewest
bypassable surfaces:** advertising and dispatch both fall out of one compiler-resolved fact
— no test to delete, no const to set wrong, no attribute to override (only the `PrettyPolicy`
resolver, shared with A, is consumer-overridable).

---

## Design C — INVERT (macro owns render; **keeps defaulted `format_pretty`**)

**Irreducible per-command author action:** per *service*, `render` attr + `impl
NormalizeRendered` (the latter is **compile-LOUD** if the attr is set but the impl missing;
the attr itself is silent if omitted → legacy path). Per *report* that wants pretty,
**override `format_pretty`** — **silent** if forgotten, because C *retains the defaulted
`format_pretty` on `OutputFormatter`* (the original root cause).

**Surviving defects:**
- **S-C1 (= U1, mass). WORST — broadest (b) of any design.** C auto-registers
  `--pretty`/`--compact` on *every* render-mode command, **and** keeps the silent default
  `format_pretty(){ self.format_text() }`. So a report that simply doesn't override gets
  `--pretty` advertised and silently equal to text — not even an explicit identical arm, just
  inherited. C reframes this as "the type chose not to differentiate (honest identity)," but
  that is a rename of (b), not its elimination: the user-visible symptom (advertised flag does
  nothing) is identical, now across dozens of commands. Only an *optional* lint is offered.
- **S-C2 (want-pretty-forgot-override).** Forget to override `format_pretty` → silent text.
  The exact original footgun, retained at report level.
- **S-C3 (new service forgets `render`).** Falls back to legacy `display_with`/`Display`
  path → silent no pretty.
- **S-C4 (d).** A param *named* `root` that isn't the config root → resolves config against
  the wrong path (C's own "mis-selection" risk); or positional target without
  `#[param(config_root)]` → cwd. Silent-degraded.
- **S-C5 (= U2).** print-and-exit body.
- **S-C6 adversarial.** Omit `render` (silent legacy); edit the single blanket
  `CliTextRender` impl (affects all, but reviewable).

**Survivor count: ~6** (S-C1..S-C6). **Worst:** S-C1.
**(a) and (c): impossible by construction** *for render-mode services*. But C is the weakest
overall because it **declines to remove the defaulted `format_pretty`** — the precise
type-level ambiguity that lets (b) and "forgot to override" persist. It also re-creates A's
mass-advertise problem *plus* the silent-inheritance problem A avoided (A forces an explicit
identical arm; C lets you inherit silently).

---

## Design D — BUILD GUARD (keeps manual Cell architecture)

**Irreducible per-command author action:** under a globals-declaring service, declare
`pretty: bool, compact: bool` params — **compile-LOUD** (Layer 1 `compile_error!`) if
forgotten. *But* the body must still call `resolve_pretty(root,…)` correctly — **silent** if
mis-written. Plus set `HAS_REAL_PRETTY` const (loud false-positive if forgotten). **D is the
only design whose primary per-command action fails LOUD** — its weakness is everything
downstream of param-presence.

**Surviving defects:**
- **S-D1 (d). WORST — the whole (d) class is wide open in pure-D.** Layer 1 forces param
  *presence*, not resolution *correctness*. A command passes the guard and still does:
  ```rust
  pub fn stats(&self, /*…*/, pretty: bool, compact: bool) -> Result<…> {
      self.pretty.set(pretty);          // ignores root AND TTY — passes Layer 1
      Ok(build_stats_data(/*…*/))
  }
  ```
  `--pretty` works, but config-at-root and TTY auto-detect are silently dead. The macro
  never sees the body. (D itself says only `CliGlobals` centralisation fixes this.)
- **S-D2 (b).** Layer 2 marker test is a deletable test; `HAS_REAL_PRETTY` is a
  hand-maintained claim (set-wrong → silent false negative).
- **S-D3 (c).** Layer 3 is fixture-dependent: an under-populated fixture makes pretty==text
  *legitimately* (false pass after you allow-list it); a mis-wire that breaks **only TTY
  auto-detect** is invisible to a non-TTY CI run; and the test is deletable.
- **S-D4 (guard is conditional on opt-in).** A developer who wants pretty but never adds
  `global = [pretty, compact]` to the impl → Layer 1 **never fires**, nothing advertised,
  silent no pretty. The guard only governs services that already opted into globals.
- **S-D5 (= U1).** Even with the marker `true`, `format_pretty==format_text` only fails
  Layer 3 if fixtured structurally; else survives.
- **S-D6 adversarial.** Delete Layer 2/3 tests (greppable but deletable); set
  `HAS_REAL_PRETTY` wrong; or escape Layer 1 via the explicit `#[cli(no_globals)]`. The
  Layer 1 `compile_error!` itself is **non-bypassable** without editing `server-less-macros`
  (and the trybuild case goes red if you do) — genuinely strong for that one shape.

**Survivor count: ~6** (S-D1..S-D6). **Worst:** S-D1 (resolution unfixed — the entire (d)
class).
**(a): caught at compile time via the param-presence proxy** (strong, non-bypassable except
`no_globals` / not-declaring-globals). **(b),(c),(d): all survive** as deletable tests /
hand-maintained const / unfixed resolution. D detects more than it prevents.

---

## Scoreboard

| design | survivors | worst survivor | (a) | (b) | (c) | (d) | bypassable surfaces |
|---|---|---|---|---|---|---|---|
| **A** subtract | **4** (+mass-advt) | U2 bypass | impossible | residual U1 + mass-advertise; §3-CI test | impossible | render_root silent-degraded | distinctness test, resolver, lint |
| **B** type-property | **5** | U1 identical-body | impossible | **eliminated** (advertise⇔capability); residual U1 only | impossible | pretty_root silent-degraded | resolver only (no test/const/attr) |
| **C** invert | **~6** | U1 mass + silent default | impossible* | **retained/renamed** (default kept, mass-advertise) | impossible* | mis-selection silent-degraded | render attr, default impl, lint |
| **D** build-guard | **~6** | (d) resolution unfixed | compile (proxy) | deletable test + const | fixture-fragile test | **unfixed** | every guard but Layer 1 |

\* C's "impossible" holds only inside render-mode services and *only for dispatch* — its
retained defaulted `format_pretty` keeps the report-level (b)/forgot-override footgun alive.

**Ranking (fewest *severe* survivors first):**
1. **B** — only design that eliminates false-advertising (b) structurally; its extra
   survivors (S-B1/U1, S-B2) are the universal semantic residual + one silent omission, both
   shared in spirit by the others. Fewest bypassable surfaces.
2. **A** — ties B on (a)/(c)=impossible, slightly *better* than B on "forgot the per-command
   step" (render is compile-required, no separate optional impl), slightly *worse* on (b)
   (mass-advertises). Ships the distinctness test B lacks.
3. **D** — strongest single guarantee (the (a) `compile_error!`) but leaves (b)/(c)/(d) as
   deletable/fragile tests and does not fix resolution at all. Detects, doesn't prevent.
4. **C** — weakest: declines to remove the defaulted `format_pretty` (the root cause),
   re-creating the report-level (b) and the mass-advertise problem with silent inheritance.

Counts alone mislead (B's 5 > A's 4 only because B adds the *silent forgot-impl* surface
that A's compile-required render closes; but B closes the *mass-advertise* surface A leaves
open). Read severity, not the integer.

---

## Verdict on grafting D's `compile_error!(a)` regardless of winner

**The graft is valuable as a fast INTERIM bridge, not as permanent belt-and-suspenders.**

- **Is it redundant under A/B/C?** Yes — *as written*. D's Layer 1 keys off two tokens:
  the impl-level `global = [pretty, compact]` list (G) and per-method `pretty: bool,
  compact: bool` params (P), firing on `G ∧ ¬P`. **A, B, and C all delete both G and P** —
  advertising becomes intrinsic/type-driven and the per-method flag params are removed. So
  after any redesign lands, Layer 1 has no subject matter: nothing declares `global=[pretty,
  compact]`, no method declares the params. It isn't a second safety net over the redesign;
  it inspects a pattern the redesign abolished. It would become dead code.
- **Is it valuable now?** Yes, decisively. The redesigns require a cross-repo server-less
  change (publish-then-bump, no path deps) plus an ~80-report / ~12-service migration —
  weeks. D's Layer 1 is ~a day, lands in server-less + the 8 BROKEN fixes, and converts the
  exact regression that recurred (stats, subagents, architecture, cross_repo_health, 4×
  rank) into a **build break today**. It buys correctness during the migration window at
  bounded cost.

**Recommendation:**
1. **Graft D's Layer 1 `compile_error!(a)` now**, as the interim regression-killer + the 8
   fixes. Treat it as scaffolding: schedule its removal as an explicit step of whichever
   redesign wins (retire-don't-deprecate — don't let it linger as dead token-inspection once
   G/P are gone).
2. **Adopt D's Layer 3 behavioural distinctness test PERMANENTLY, regardless of winner.**
   This — not Layer 1 — is the real belt-and-suspenders, because it is the only mechanism
   that closes the two universal residuals **U1** (pretty bytes == text bytes) and **U2**
   (body bypasses the render path) that *no* redesign closes via types. If the winner is B
   or C (neither ships a distinctness test), grafting Layer 3 is mandatory, not optional. If
   the winner is A (which has §3-CI), Layer 3's against-the-binary form additionally covers
   U2, which §3-CI's in-process distinctness check does not.

Net: **graft the (a)-guard as interim and plan its deletion; graft the behavioural test as
permanent.** The durable cross-cutting protection is the behavioural test, not the compile
error.
