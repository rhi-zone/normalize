# Design B — pretty-ness as a TYPE PROPERTY the macro consumes at compile time

**Date:** 2026-06-28
**Frame:** different conceptual primitive — make rendering capability a property of the
report TYPE, derived from one type-level fact, so that **both** flag-advertising and
output-dispatch fall out of that fact and the compiler enforces correctness. Retire the
hand-written `pretty: Cell<bool>` + `resolve_pretty` + `display_with` plumbing.
**Companions:** `diagnosis.md`, `pretty-wiring-audit.md`.

This document designs; it does not implement. The one claim everything rests on — that a
single type-level fact can drive both advertising and dispatch through compiler-resolved
generated code — was verified with a standalone `rustc` probe (see §7).

---

## 0. The crux: what can a proc macro actually SEE?

A `#[cli]` proc macro sees **tokens**, never resolved types. It cannot ask "does
`SessionAnalysisReport` implement `PrettyFormat`?" — trait resolution happens in a later
compiler phase. So a naive reading of this frame ("the macro looks at the report type and
KNOWS whether it has pretty") is **infeasible as stated**. The macro cannot branch its
*emitted tokens* on a trait bound of the return type.

What the macro *can* do, and what this design exploits:

1. It can see the return-type **token** (`inner_ty` — the `Ok` type of the `Result`).
   `crates/server-less-macros/src/cli.rs:1910-1917` already extracts exactly this for
   default-`Display` detection.
2. It can emit **generic code** whose correctness the *compiler* resolves against that
   token — including code that, via Rust's method/associated-item resolution rules,
   behaves differently depending on whether the type implements a capability trait.
3. It can read **parameter-name tokens** (e.g. a param literally named `root`).
4. The clap command-tree is built at **runtime**, so "advertising" a flag is a runtime
   `if`, not a macro-time decision — meaning advertising *can* be gated on a value the
   compiler computed from the type.

So the honest restatement of the frame, and the thesis of this design:

> Pretty-ness becomes a single type-level fact — `impl PrettyFormat for Report` — and the
> macro emits **the same generic code for every command**. The **compiler**, not the
> macro, resolves that code against each return type, driving advertising and dispatch
> identically. The macro never "knows"; the generated-code-the-compiler-checks does.

This is strictly stronger than a compile-error guardrail: the defective states become
**unrepresentable** because the per-command plumbing that could be wrong is *deleted*.

---

## 1. The mechanism

### 1.1 Split the capability into its own trait (the single fact)

Today (`normalize-output`, re-exported via `crates/normalize/src/output.rs`):

```rust
pub trait OutputFormatter {
    fn format_text(&self) -> String;
    fn format_pretty(&self) -> String { self.format_text() }  // DEFAULTED — the footgun
}
```

The default is the root of defect class (a)/(b): every type "has" a `format_pretty` (it
just silently equals text), so neither the type system nor any tooling can distinguish
"has real pretty" from "doesn't." We remove the default and lift the capability into a
sub-trait:

```rust
pub trait OutputFormatter {
    fn format_text(&self) -> String;            // unchanged, still required
}

/// A report whose pretty rendering is genuinely distinct from its text rendering.
/// Implementing this trait IS the type-level fact "this report has pretty output."
pub trait PrettyFormat: OutputFormatter {
    fn format_pretty(&self) -> String;          // non-defaulted: implementing it is a deliberate act
}
```

Now `Report: PrettyFormat` is a binary, type-level fact. There is no "trivial pretty"
state — a type either declares real pretty output (by implementing the trait) or it does
not exist as a `PrettyFormat`. The no-stub rule (`CLAUDE.md`: "`None`/empty is only
correct when the concept genuinely doesn't exist") is honoured: text-only reports simply
don't implement `PrettyFormat`, exactly as Bash reports don't implement a type system.

> **Rejected variant — non-defaulted `format_pretty` on `OutputFormatter` itself.** The
> brief asks us to evaluate this. It fails the no-stub rule: it would force every one of
> the ~80 text-only report types to write `fn format_pretty(&self){ self.format_text() }`
> — re-introducing the exact "trivial pretty == text" ambiguity we are removing, now as
> mandatory boilerplate. Splitting into `PrettyFormat` is the same idea done right: the
> capability is opt-in and its presence is meaningful.

### 1.2 Two compiler-resolved probes (the generic code the macro emits)

The macro emits identical code for every command. Rust's resolution rules make that code
behave per-type. Two well-known specialization patterns carry the two halves:

**Dispatch — inherent-method-beats-trait-method specialization:**

```rust
// in server-less
pub struct Render<'a, T>(pub &'a T);

impl<T: PrettyFormat> Render<'_, T> {            // inherent method: HIGHER resolution priority
    pub fn render(&self, want_pretty: bool) -> String {
        if want_pretty { self.0.format_pretty() } else { self.0.format_text() }
    }
}
pub trait RenderFallback { fn render(&self, want_pretty: bool) -> String; }
impl<T: OutputFormatter> RenderFallback for Render<'_, T> {   // trait method: fallback
    fn render(&self, _want_pretty: bool) -> String { self.0.format_text() }
}
```

The macro emits, at the display site, **one line** regardless of type:

```rust
let __display = ::server_less::Render(&value).render(__want_pretty);
```

For a `PrettyFormat` type this resolves to the inherent method (which can call
`format_pretty`); for any other `OutputFormatter` it resolves to the trait fallback (text
only). Inherent methods always win over trait methods — the compiler picks.

**Advertising — inherent-const-beats-trait-const specialization:**

```rust
// in server-less
pub struct PrettyProbe<T>(core::marker::PhantomData<T>);
impl<T: PrettyFormat> PrettyProbe<T> { pub const HAS_PRETTY: bool = true; }   // inherent
pub trait ProbeFallback { const HAS_PRETTY: bool = false; }
impl<T: OutputFormatter> ProbeFallback for PrettyProbe<T> {}                  // fallback
```

The macro emits, at the clap-builder site for each leaf command (which runs at runtime):

```rust
if <::server_less::PrettyProbe<#inner_ty>>::HAS_PRETTY {
    cmd = cmd.arg(/* --pretty */).arg(/* --compact */);
}
```

`#inner_ty` is the return-type token the macro already has. The boolean is computed by the
compiler from the single fact `#inner_ty: PrettyFormat`. **Advertising and dispatch now
derive from the same fact, by construction.**

`global = [pretty, compact]` is **retired** — flags are no longer declared per-service;
they appear exactly on the commands whose return type is `PrettyFormat`.

### 1.3 What disappears

- `pretty: Cell<bool>` on every service struct (sessions, analyze, rank, facts, …).
- `super::resolve_pretty(...)` calls scattered across ~50 method bodies.
- `pretty: bool, compact: bool` params on ~46 methods.
- `display_output` / `display_analyze` / `display_move` / `display_node_types` and every
  other hand-written `display_with` whose only job was the text/pretty branch.
- `global = [pretty, compact]` on every `#[cli]` impl block.

The macro owns the entire pretty pathway; there is nothing per-command left to forget.

---

## 2. Concrete before/after

### 2.1 BROKEN command: `sessions stats` (the trigger case)

**Before** (`service/sessions.rs:240-339` + report at
`normalize-session-analysis/src/lib.rs:901`): the report has a rich `format_pretty`, the
service advertises `global=[pretty,compact]`, the method `display_with="display_output"`
dispatches on `self.pretty` — but `stats` never declares `pretty`/`compact` and never
calls `self.pretty.set(...)`, so `display_output` always takes the text branch. `--pretty`
is accepted and silently ignored. (Defect class **a**.)

**After:**

```rust
// normalize-session-analysis/src/lib.rs — the OutputFormatter impl loses format_pretty,
// which moves to a PrettyFormat impl. This single edit is the entire fix.
impl OutputFormatter for SessionAnalysisReport {
    fn format_text(&self) -> String { /* unchanged */ }
}
impl PrettyFormat for SessionAnalysisReport {
    fn format_pretty(&self) -> String { /* the existing write_pretty() body */ }
}
```

```rust
// service/sessions.rs — stats() loses NOTHING it needs and GAINS nothing to forget.
// No pretty/compact params, no self.pretty.set, no #[cli(display_with=...)] for the
// text/pretty split. The macro emits Render(&value).render(want) automatically.
pub fn stats(&self, /* … domain params only … */ root: Option<String>, /* … */)
    -> Result<SessionAnalysisReport, String> { /* unchanged body */ }
```

Because `SessionAnalysisReport: PrettyFormat`, the macro auto-advertises `--pretty`/
`--compact` on `sessions stats` and auto-dispatches to `format_pretty`. The bug cannot
recur: there is no wiring step to omit.

### 2.2 WORKING command: `sessions list`

**Before** (`service/sessions.rs:89-145`): declares `pretty: bool, compact: bool`, calls
`self.pretty.set(resolve_pretty(resolved_root, pretty, compact))`, uses
`display_with="display_output"`. Correct, but the correctness is hand-maintained and
copy-pasted across dozens of methods.

**After:** `SessionListReport`'s `format_pretty` moves to an `impl PrettyFormat`. The
method drops `pretty`/`compact` params, the `self.pretty.set(...)` line, and the
`display_with` attribute. ~5 lines deleted per method. Behaviour identical; the
correctness is now structural, not maintained.

Net effect of the migration is **code deletion** across the whole workspace, not
addition.

---

## 3. How each defect class becomes impossible

### (a) SILENT NO-OP (impl'd pretty never dispatched) — impossible by construction

Dispatch is the macro-emitted `Render(&value).render(want)`. Any type that is
`PrettyFormat` resolves to the inherent method that calls `format_pretty`. There is no
per-method `self.pretty.set` to forget, because there is no `self.pretty` and no method
param. The state "report has `format_pretty` but it is never called" is unrepresentable —
implementing `PrettyFormat` *is* being dispatched.

### (b) ADVERTISED NO-OP (flag advertised, no real pretty) — impossible by construction

`--pretty`/`--compact` are registered **iff** `<PrettyProbe<ReturnTy>>::HAS_PRETTY`, i.e.
iff the return type is `PrettyFormat`. A service can no longer advertise pretty for a
text-only command, because `global=[pretty]` is gone and advertising is per-command,
gated on the type. The ratchet/budget "dead `--pretty` flag" situation (audit §1b.4)
cannot exist: those reports don't implement `PrettyFormat`, so the flag is never offered.

### (c) DEAD DISPATCH (display fn calls `format_text` unconditionally) — eliminated by removal

The hand-written `display_with` fns that branched text/pretty are deleted; the macro owns
the branch. There is no longer a place to write a dispatch that ignores pretty. For the
`edit`/`node-types` cases (audit §1b.1-2) whose custom display fns hardcoded
`format_text()`, the fix is: delete the custom fn, move `format_pretty` to a
`PrettyFormat` impl, done.

### The genuine COMPILE-ERROR lever

Removing `format_pretty` from `OutputFormatter` means `value.format_pretty()` only
compiles when the receiver's type is `PrettyFormat`. Any leftover hand-written code that
calls `format_pretty` on a non-`PrettyFormat` type is now a **method-not-found compile
error**, not a silent text fallback. So the migration is self-checking: the compiler
points at every site that assumed the old default.

Honest scope of "compile error": the *defective states themselves* (a/b/c) are prevented
by construction (deletion of the plumbing), which is stronger than a guard you must
remember to write. The compiler-error is the *migration safety net* and the guard against
anyone re-adding `format_pretty` calls outside the trait.

---

## 4. Root-aware + TTY resolution flow

`want_pretty` must still be `!compact && (pretty || config_at_root.pretty.enabled())`,
where the config is rooted at the **command target** and TTY auto-detection applies. With
the per-method plumbing gone, the resolution moves to a single server-less hook that the
macro calls — and the macro supplies `root` by reading the **param-name token**.

```rust
// server-less: one hook, default = cwd-rooted resolution + TTY.
pub trait PrettyPolicy {
    /// raw_pretty/raw_compact are the --pretty/--compact flag values from sub_matches.
    /// root is the command's resolved target dir (see below).
    fn resolve_pretty(&self, root: &std::path::Path, raw_pretty: bool, raw_compact: bool) -> bool {
        // default: TTY-only, no config. normalize overrides to add config.
        !raw_compact && (raw_pretty || std::io::IsTerminal::is_terminal(&std::io::stdout()))
    }
}
```

```rust
// normalize: implemented ONCE, not per method. This is the home of resolve_pretty().
impl PrettyPolicy for NormalizeService {
    fn resolve_pretty(&self, root: &Path, p: bool, c: bool) -> bool {
        let config = NormalizeConfig::load(root);            // root-aware config
        !c && (p || config.pretty.enabled() || stdout_is_tty())
    }
}
```

**How `root` reaches the hook** — three options, in preference order:

1. **Param-name convention (recommended).** The macro already iterates `regular_params`
   and sees each param's name token (`cli.rs:1715`). When a leaf command has a param named
   `root` (the universal convention in this codebase — every rooted command uses
   `root: Option<String>`), the macro passes its resolved value to
   `self.resolve_pretty(root.as_deref().unwrap_or(Path::new(".")), raw_pretty, raw_compact)`.
   A `#[param(pretty_root)]` marker can make the binding explicit where the param isn't
   named `root`. Zero per-method code; the macro reads tokens it already has.
2. **Default to cwd.** If no `root`-named param exists, resolve against `.`. TTY still
   works (process-global); only *config-file* pretty defaults from a non-cwd `--root`
   would be missed — a minor, documented regression that the audit already anticipated
   (`pretty-wiring-audit.md:198-203`). In practice `--root` rarely points outside cwd and
   `pretty.enabled()` rarely varies by root.
3. **Explicit attribute** `#[cli(pretty_root = "root")]` for the rare command whose target
   dir is in a differently-named param.

The macro then emits, before display:

```rust
let __raw_pretty   = sub_matches.get_flag("pretty");    // registered only if HAS_PRETTY
let __raw_compact  = sub_matches.get_flag("compact");
let __want_pretty  = <Self as ::server_less::PrettyPolicy>::resolve_pretty(
    self, /* root from param token or "." */, __raw_pretty, __raw_compact);
```

Resolution stays exactly where the root is known (the leaf, via its `root` param token),
but is emitted by the macro instead of hand-written — so it cannot be forgotten, and the
TTY path is no longer bypassed for commands that previously omitted the params (the
`stats` "always text even in a terminal" symptom, `diagnosis.md:126-129`, is fixed for
free).

---

## 5. Migration plan, cost, blast radius

### Order (finish-before-build, per CLAUDE.md)

1. **server-less:** add `PrettyFormat` is normalize-side; in server-less add `Render`,
   `PrettyProbe`, the `PrettyPolicy` hook, the param-`root` detection, and emit the
   advertise-`if` + `Render(...).render(...)` in `generate_leaf_match_arm` /
   builder. Keep the legacy `display_with` path working for non-pretty custom formatting
   (it still has uses — see §6). Ship behind nothing; it's additive to server-less.
2. **normalize-output:** split `OutputFormatter` / `PrettyFormat`; remove the default.
   This is the breaking change — it will not compile until step 3 is done. Do it in one
   commit with step 3.
3. **normalize + feature crates:** for every type that currently overrides
   `format_pretty`, move that method into an `impl PrettyFormat`. Delete `pretty: Cell`,
   `resolve_pretty` calls, `pretty`/`compact` params, `display_with` text/pretty fns, and
   `global=[pretty,compact]`. The compiler enumerates every site for you (method-not-found
   on stale `format_pretty` calls).
4. **Tests:** keep `assert_output_formatter::<T>()`; add `assert_pretty_format::<T>()` for
   the pretty set as documentation (optional — auto-advertising already enforces it).
5. Update `docs/cli/`, `README.md`, `LLMS.md`, `docs/cli-design.md`, `CHANGELOG.md`.

### Cost

- **~16 "real pretty" report types** (the working set's distinct `format_pretty`
  overrides + the 8 broken ones): mechanical move of an existing method body into an
  `impl PrettyFormat` block. Each ~2 lines of structural edit, no logic change.
- **~46 working methods:** net deletion (~5 lines each): drop params, `set`, attribute.
- **8 broken + 7 unreachable (edit/node-types) commands:** fixed as a side-effect of the
  same move; delete their dead custom display fns.
- **~80 text-only report types:** **no change** — they implement only `OutputFormatter`,
  never used `format_pretty`, and simply stop receiving the (now-removed) default.

### Blast radius — the breaking change, quantified

Removing `format_pretty` from `OutputFormatter` is a **breaking change to the
`normalize-output` public trait**. Every server-less/normalize-output consumer that
*overrode* `format_pretty` must switch to `impl PrettyFormat`; consumers that only
implemented `format_text` are unaffected (the vast majority).

From the audit, real `format_pretty` overrides are **concentrated in the main `normalize`
crate** + `normalize-session-analysis`. Surveyed feature crates that impl
`OutputFormatter` — `normalize-ratchet`, `normalize-budget`, `normalize-rules`,
`normalize-facts`, `normalize-knowledge-graph`, `normalize-native-rules`,
`normalize-syntax-rules` — have **no real pretty** (audit §1, N/A list): they implement
only `format_text`, so they need **no source change** and only recompile. Blast radius
outside the main crate is therefore near-zero in practice, even though the trait change is
formally breaking.

External (non-normalize) server-less consumers: any third party that overrode
`format_pretty` breaks at the trait split. Per `CLAUDE.md`, server-less is our own
dogfooded project and normalize is effectively its sole serious consumer; the SemVer bump
(server-less + normalize-output minor/major) is the cost. "Retire, don't deprecate"
(throughlines) favours the clean split over carrying a defaulted method forever.

### Risk

- **Medium.** The trait split touches a foundational crate; the compiler makes the
  migration mechanical but wide. The macro change is localized to two emission sites.
- The one-commit "won't compile until done" window for steps 2-3 is real; do it in a
  single change, lean on `cargo check` to drive the worklist.

---

## 6. Honest trade-offs and the hard token-only limit

- **The macro never "knows" the type's capability.** It emits identical generic code; the
  *compiler* resolves it. This is the unavoidable consequence of token-only macros and is
  the precise sense in which the frame's literal phrasing is infeasible. The design is
  honest about this: the type-level fact lives in the trait system and is consumed by
  *generated-code-the-compiler-checks*, not by macro-time branching.

- **Specialization via inherent-vs-trait priority is a "trick."** Both `Render::render`
  (inherent method beats trait method) and `PrettyProbe::HAS_PRETTY` (inherent assoc const
  beats trait assoc const) rely on Rust's resolution-priority rules rather than
  `min_specialization` (unstable). It is stable and verified (§7), but it is subtle;
  maintainers unfamiliar with the pattern may find it surprising. Mitigation: confine it
  to server-less with a thorough doc-comment and a regression test like §7.

- **Custom `display_with` does not vanish entirely.** Commands that render *neither* plain
  text nor pretty but something bespoke (e.g. `translate`, `view trace`, syntax dumps —
  audit's "display fn never dispatches" list) still use `display_with`, and that's
  correct: they have no pretty to lose. The design only removes `display_with` fns whose
  *sole* purpose was the text/pretty branch. A `display_with` fn that wants to *also*
  honour pretty must call through `PrettyFormat` (compile-checked) — it cannot silently
  drop it.

- **Advertising is the more fragile half.** Dispatch (value in hand) is rock-solid.
  Advertising relies on `<PrettyProbe<T>>::HAS_PRETTY` resolving the inherent const
  through a generic probe — verified working in §7, but it is the least-trodden path. If a
  future compiler change perturbed inherent-assoc-const priority, the fallback is benign:
  advertise `--pretty` on every leaf (reverting (b) to a harmless dead flag) or declare
  pretty in the attribute. Dispatch correctness is never at risk.

- **`Render` adds a `&`-wrapper allocation-free shim** per display call — negligible.

---

## 7. Verification (the load-bearing claim)

A standalone `rustc --edition 2021` probe confirmed both halves resolve the type-level
fact at the call site, compiler-checked, with zero per-type wiring:

```
Rich  want=true  -> PRETTY     (PrettyFormat type, pretty requested)
Rich  want=false -> text       (PrettyFormat type, pretty not requested)
Plain want=true  -> plain      (non-PrettyFormat type: falls back to text)
Rich  HAS_PRETTY  = true       (advertising probe: type-derived)
Plain HAS_PRETTY  = false      (advertising probe: type-derived)
```

`Rich` implements `PrettyFormat`; `Plain` implements only `OutputFormatter`. Neither the
dispatch site (`Render(&v).render(want)`) nor the advertising site
(`<PrettyProbe<T>>::HAS_PRETTY`) names the capability — the compiler selects the right
behaviour from the single fact `impl PrettyFormat for Rich`.

---

## 8. Summary

| | mechanism | who enforces |
|---|---|---|
| single type-level fact | `impl PrettyFormat for Report` (sub-trait of `OutputFormatter`, non-defaulted) | the author, deliberately |
| dispatch | macro emits `Render(&value).render(want)`; inherent method beats trait fallback | compiler (method resolution) |
| advertising | macro emits `if <PrettyProbe<RetTy>>::HAS_PRETTY { add --pretty/--compact }` | compiler (assoc-const resolution) + runtime clap builder |
| root+TTY | macro reads `root` param token → `self.resolve_pretty(root, raw_pretty, raw_compact)` via `PrettyPolicy` hook | server-less hook, normalize impl |
| (a) silent no-op | no `self.pretty.set` to forget — dispatch is automatic | impossible by construction |
| (b) advertised no-op | flag gated on `HAS_PRETTY` — can't advertise without `PrettyFormat` | impossible by construction |
| (c) dead dispatch | text/pretty `display_with` fns deleted — macro owns the branch | impossible by construction |
| migration safety net | stale `format_pretty` calls are method-not-found | compile error |
