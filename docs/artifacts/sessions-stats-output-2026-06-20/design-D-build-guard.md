# Design D — Build-time guard / exhaustiveness

**Frame:** Keep the runtime dispatch architecture as-is (manual `pretty` Cell, or the
minimal `CliGlobals` hook). Make each of the three pretty-output defect classes either a
**compile error** or a **deterministic CI failure** via a *guard layer* — not a trait
redesign. Push the lint/test/CI arm as hard as it goes, and be honest about exactly where
it stops.

**Companion docs:** `diagnosis.md` (the `sessions stats` instance), `pretty-wiring-audit.md`
(the full BROKEN/WORKING/N-A census — 8 strict-BROKEN, ~46 working, plus 7 adjacent
unreachable-pretty display fns).

---

## 0. What is knowable at each stage (the load-bearing fact)

Everything in this frame follows from what information exists *where*. I verified each of
these against the source.

At **macro-expansion time**, `#[cli]` (`server-less-macros/src/cli.rs`) knows, per method:

- **G** — does the impl declare `global = [pretty, compact]`? (`args.global` → `global_flags`,
  cli.rs:607–611).
- **P** — does the method declare `pretty: bool` *and* `compact: bool` params? (scan
  `regular_params` for those names + `is_bool`; the param list is fully parsed —
  `ParamInfo { name, is_bool, .. }`, parse/lib.rs:60–87).
- **D** — does the method carry `#[cli(display_with = "fn")]`, and what is the fn *name*?
  (`get_display_with`, cli.rs:483).
- **R** — the report type (`return_info.ok_type`, cli.rs:1666).

The macro does **not** know:

- the **body** of the `display_with` fn (whether it actually dispatches on `self.pretty`),
- whether the report type **overrides** `format_pretty()` or inherits the **default**
  (`OutputFormatter::format_pretty` has a default body that calls `format_text`,
  normalize-output/src/lib.rs:100–102 — so `<R as OutputFormatter>` type-checks identically
  whether or not a real override exists),
- whether the method body calls `resolve_pretty(root, …)` correctly (root-aware + TTY).

This split is the whole design. It dictates which defect is compile-time-killable and which
is only CI-detectable.

Mapping to the three defect classes:

| defect | description | macro sees enough? | strongest guard | stage |
|---|---|---|---|---|
| **(a)** silent no-op | G true, P false → flag parses, value never reaches body | **YES** (G ∧ ¬P is fully visible) | `compile_error!` in the macro | **compile** |
| **(b)** advertised no-op | G true, but no report under the service has a *real* `format_pretty` | **NO** (default impl hides override) | marker const + inventory test, or behavioural test | **CI** (test) |
| **(c)** dead dispatch | `display_with` fn calls `format_text()` unconditionally; real `format_pretty` is dead | **NO** (fn body opaque) | behavioural `--pretty` vs default snapshot | **CI** (test) |

There is a clean, honest line here: **(a) is the only one of the three that is genuinely
compile-time-enforceable.** (b) and (c) require a test, and that test has irreducible
reliability limits (below). Anyone claiming all three are "compile errors" is wrong about
(b)/(c).

---

## 1. The guard layers

Three layers, in decreasing order of strength. Layer 1 is the centrepiece.

### Layer 1 — Macro `compile_error!` for defect (a)  *(compile-time, non-bypassable)*

Extend `cli.rs`'s `expand` with a guard that runs after `global_flags` is computed
(cli.rs:611) and after `partition_methods` (cli.rs:631). There is already a precedent for
exactly this shape: `check_reserved_flag_collisions` (cli.rs:636) emits a `syn::Error`
spanned to a parameter when a param name collides with an injected global. We add a sibling:

```rust
// server-less-macros/src/cli.rs, new fn called from expand() after line 636.
fn check_global_pretty_wiring(
    partitioned: &PartitionedMethods,
    global_flags: &[String],
) -> syn::Result<()> {
    // Only fires for the pretty/compact pairing this guard governs.
    let governs = global_flags.iter().any(|g| g == "pretty")
        && global_flags.iter().any(|g| g == "compact");
    if !governs {
        return Ok(());
    }
    let mut errors: Option<syn::Error> = None;
    for m in &partitioned.leaf {
        if has_cli_no_globals(m) {            // explicit per-method opt-out
            continue;
        }
        let declares = |name: &str| {
            m.params.iter().any(|p| p.is_bool && p.name_str() == name)
        };
        if !(declares("pretty") && declares("compact")) {
            let e = syn::Error::new_spanned(
                &m.name,
                format!(
                    "`{}` is in an impl that declares `global = [pretty, compact]`, but does \
                     not declare `pretty: bool, compact: bool` parameters. The --pretty/--compact \
                     flags will parse but be silently ignored (the value never reaches the method \
                     body). Add both params and call `resolve_pretty(root, pretty, compact)`, or \
                     annotate the method `#[cli(no_globals)]` to opt out.",
                    m.name_str()
                ),
            );
            match &mut errors { Some(acc) => acc.combine(e), None => errors = Some(e) }
        }
    }
    match errors { Some(e) => Err(e), None => Ok(()) }
}
```

**What the developer sees — guard FIRING (broken `sessions stats` as it exists today):**

```
error: `stats` is in an impl that declares `global = [pretty, compact]`, but does not
       declare `pretty: bool, compact: bool` parameters. The --pretty/--compact flags
       will parse but be silently ignored (the value never reaches the method body). Add
       both params and call `resolve_pretty(root, pretty, compact)`, or annotate the
       method `#[cli(no_globals)]` to opt out.
   --> crates/normalize/src/service/sessions.rs:242:12
    |
242 |     pub fn stats(
    |            ^^^^^
```

**Guard PASSING — working `sessions list`:** `list` declares `pretty: bool, compact: bool`
(sessions.rs:115–116), so `declares("pretty") && declares("compact")` is true → no error.

This is the strong result. For defect (a), the property "flag advertised but value never
reaches the body" becomes a **hard compile error** at the exact method span. It is
**non-bypassable in the strongest sense available**: the guard lives inside the same proc
macro that *generates the entire CLI*. You cannot keep the command and skip the guard — the
command does not exist without running the macro. There is no `#[allow]`, no feature flag, no
test to delete. The only escape is the explicit, greppable, reviewable `#[cli(no_globals)]`.

**What it does NOT do:** it forces param *presence*, not resolution *correctness*. A dev can
satisfy the guard with `pretty: bool, compact: bool` and then write `self.pretty.set(pretty)`
(dropping `root`/TTY) inside the body. The macro cannot see the body, so it cannot verify
`resolve_pretty(root, …)` is called. See §4.

**Trybuild — guarding the guard.** Add `server-less-macros` trybuild cases so the guard
itself can't silently regress:

- `tests/ui/global_pretty_missing_params.rs` — impl with `global = [pretty, compact]` and a
  method missing the params → `.stderr` asserts our exact message. (Compile-FAIL expected.)
- `tests/ui/global_pretty_opt_out.rs` — same, but method has `#[cli(no_globals)]` → compiles.
  (Compile-PASS expected.)
- `tests/ui/global_pretty_wired.rs` — method declares both params → compiles.

trybuild does **not** detect (b) in normalize. What it detects is that *the macro's (a)
detector is intact*: if a future server-less refactor drops the guard, `missing_params.rs`
flips from fail→pass and the trybuild test goes red. trybuild guards the guard; it is not a
guard on normalize's own commands.

### Layer 2 — Generated inventory + exhaustiveness test  *(CI; catches (a) defence-in-depth, surfaces (b))*

The current `assert_output_formatter` test (output.rs:74–374) is a **hand-maintained** list
of ~150 `assert_output_formatter::<T>()` calls. It already shows drift ("Task 4: missing
entries", output.rs:245). A hand-maintained enumeration is not exhaustiveness — it is a list
someone has to remember to update. Replace the *enumeration mechanism* with one the macro
emits, so it cannot drift.

**Macro emits a manifest entry per `#[cli]` method** using the `inventory` crate (chosen
because it auto-collects across the many crates that link into the `normalize` binary — a
`pub const` per service would re-introduce a hand-maintained collection point in the test):

```rust
// server-less: new public type, registered once.
pub struct CliCommandMeta {
    pub path: &'static str,            // "sessions stats"
    pub advertises_pretty: bool,       // G: impl declares global pretty/compact
    pub declares_pretty_params: bool,  // P: method declares the params
    pub has_display_with: bool,        // D
    pub report_type: &'static str,     // R, for diagnostics
}
inventory::collect!(CliCommandMeta);
```

The macro, inside `generate_leaf_match_arm`/`expand`, emits one `inventory::submit! {
CliCommandMeta { … } }` per leaf method, filling G/P/D/R — all of which it already computes.

**The exhaustiveness test (in `normalize`, runs under `cargo test`):**

```rust
#[test]
fn every_command_with_global_pretty_wires_its_params() {
    for meta in inventory::iter::<CliCommandMeta>() {
        if meta.advertises_pretty {
            assert!(
                meta.declares_pretty_params,
                "{} advertises --pretty but does not declare pretty/compact params \
                 (report {}); flag is a silent no-op [defect (a)]",
                meta.path, meta.report_type,
            );
        }
    }
}
```

Because entries are *generated*, every new `#[cli]` command is automatically in the
manifest. There is no list to forget. This is the real "exhaustiveness" upgrade over the
status-quo test.

**This layer also catches (b) — partially — *if* we add a report-side marker.** Because the
default `format_pretty` hides overrides from the type system, the only way to know a report
has a *real* pretty is to record it explicitly. Add to `OutputFormatter`:

```rust
/// True when this type provides a non-trivial format_pretty distinct from format_text.
/// Override to `true` when you implement a real format_pretty.
const HAS_REAL_PRETTY: bool = false;
```

The macro emits `has_real_pretty: <R as OutputFormatter>::HAS_REAL_PRETTY` into the manifest.
The test then asserts the **(b)** direction:

```rust
// (b): a service advertises --pretty but NO report under it has a real pretty → dead flag.
let by_service = group_by_service(inventory::iter::<CliCommandMeta>());
for (service, cmds) in by_service {
    if cmds.iter().any(|c| c.advertises_pretty)
        && !cmds.iter().any(|c| c.has_real_pretty) {
        panic!("service `{service}` advertises --pretty but no report implements a real \
                format_pretty — the flag is advertised-but-no-op [defect (b)]");
    }
}
```

**Honest cost of the marker:** `HAS_REAL_PRETTY` must be set on every report that *has* a
real pretty (the ~46 working + the 7 adjacent). If a dev writes a real `format_pretty` but
forgets the const, the (b) check produces a *false positive* (claims no real pretty) — which
is at least loud, not silent. The deeper limit: the const is a hand-maintained claim. It
trades "forget to wire the flag" (defect (a), now compile-caught) for "forget to set the
const." It is strictly better than nothing because the failure is a CI panic, not a silent
text fallback — but it is **not** type-derived truth.

### Layer 3 — Behavioural snapshot test for defect (c)  *(CI; the only reliable (c) catch)*

Defect (c) — `display_with` calls `format_text()` unconditionally — is invisible to both the
macro (opaque body) and the inventory marker (the report *does* have a real pretty; it is the
*bridge* that ignores it). The only thing that observes it is **actually running the command
both ways and comparing**.

Drive the real CLI over a committed fixture corpus (a tiny synthetic session/repo under
`tests/fixtures/`), for each command whose manifest entry has `has_real_pretty && advertises_pretty`:

```rust
#[test]
fn pretty_differs_from_text_for_commands_with_real_pretty() {
    for meta in inventory::iter::<CliCommandMeta>() {
        if !(meta.advertises_pretty && meta.has_real_pretty) { continue; }
        let text   = run_cli(meta.path, &fixture_args(meta), &["--compact"]);
        let pretty = run_cli(meta.path, &fixture_args(meta), &["--pretty"]);
        assert_ne!(
            strip_ansi(&pretty), strip_ansi(&text),
            "{}: --pretty output is identical to text — display path never calls \
             format_pretty [defect (c)] (or resolution not wired [defect (a)])",
            meta.path,
        );
    }
}
```

Comparing **after** `strip_ansi` is deliberate: a difference that is *only* ANSI color is not
proof the pretty *structure* (bar charts, `━━━` headers) is reached. Requiring a structural
difference is what makes this a real (c) catch rather than a color check.

**This is also the most complete catch for (a)** — it observes the end-to-end effect, not the
proxy (param presence). A command that passes Layer 1 (params present) but mis-wires
resolution (Layer 1's blind spot, §4) will *still fail here* if the mis-wiring makes
`--pretty` produce text. So Layer 3 backstops Layer 1's resolution gap.

**Honest limit (the hard one):** the test needs a fixture that *populates* the report enough
that pretty actually diverges from text. `format_pretty` on an *empty* report frequently
equals `format_text` (an empty bar chart renders like an empty table). So:

- **False pass:** if the fixture under-populates a command, pretty==text legitimately, the
  `assert_ne!` would *fail* — so we must curate `fixture_args` per command to hit the
  populated path. That per-command curation is the real cost and the real fragility.
- **False fail risk:** none in the populated case, but a command with a genuinely
  near-identical pretty (mostly-color) would need an allow-list entry, which is a maintenance
  seam.

This is why Layer 3 is honestly labelled CI-time-and-fixture-dependent, not "impossible by
construction."

---

## 2. How close to impossible-by-construction, per defect

| defect | guard | closeness to impossible-by-construction | residual gap |
|---|---|---|---|
| **(a)** | Layer 1 `compile_error!` | **Very high.** Detection is type/structure-level, lives inside the CLI-generating macro, no opt-out but the explicit `#[cli(no_globals)]`. As strong as a guard gets without owning the body. | Forces param *presence*, not resolution *correctness* (§4). Closed in practice by Layer 3. |
| **(b)** | Layer 2 marker + inventory test | **Medium.** CI-time. Truth depends on the `HAS_REAL_PRETTY` const being set honestly. | Const is a hand-maintained claim; default `false` means a forgotten override is a loud false-positive, a forgotten *real* pretty under a non-advertising service is undetected. Test is deletable (see below). |
| **(c)** | Layer 3 behavioural snapshot | **Low–medium.** CI-time, fixture-dependent. Reliable only where the fixture populates the report. | Empty-data false-equality; per-command fixture curation; test/allow-list maintenance; test is deletable. |

**On "the guard is a test a dev could delete."** Real, and the honest weak point of Layers
2–3. Mitigations, in order of strength:

1. **Layer 1 needs no such mitigation** — it is not a test. It is macro output. Deleting it
   means editing `server-less-macros`, which a normalize dev does not do casually, and the
   trybuild case turns red if they do.
2. For Layers 2–3: the inventory *manifest* is macro-emitted, so the **data** can't drift;
   only the *assertion* file is deletable. Keep both tests in one `#[test]`-dense module that
   the pre-commit hook + CI both run; a deletion shows up as a coverage drop in the diff and
   is reviewable. This is governance, not a hard guarantee — state it as such.
3. Optionally promote the Layer 2 (a)-direction assertion into Layer 1 instead (it already
   is), so the *only* thing a test-deletion loses is (b) and (c) — never (a).

---

## 3. The inventory question, answered concretely

> Does the macro have a registry of all commands? Could it generate a manifest a test
> iterates?

**Today:** no `inventory` usage anywhere in server-less (verified). The closest existing
runtime registry is `cli_command()` (cli.rs:1293), which returns a fully-built clap
`Command` tree — a test *can* walk it to enumerate subcommands and read which advertise
`--pretty` (the **advertised/G** side comes for free from introspecting clap). But
`cli_command()` carries no report-type or `format_pretty` information, so it can establish G
but not P-as-typed, R, or `HAS_REAL_PRETTY`.

**Feasible upgrade:** the macro already computes G, P, D, R at expansion — emitting an
`inventory::submit!` per leaf method is a small, additive change (one `quote!` block in
`expand`). `inventory` is the right collector because normalize's commands are spread across
many crates that all link into one binary/test; a generated `const` slice per crate would
force the test to name every crate's slice by hand — re-creating the drift the design is
trying to kill. With `inventory`, the test does `for m in inventory::iter::<CliCommandMeta>()`
and gets *every* command in the linked graph with zero hand-maintenance. (Caveat:
`inventory` uses life-before-main constructors; fine for a CLI/test binary.)

This is the concrete mechanism that makes "exhaustiveness" real rather than aspirational, and
it directly retires the hand-maintained-list smell in the current `assert_output_formatter`.

---

## 4. Root-aware + TTY resolution: fix or only detect?

**Layer 1 only *detects the precondition*.** It guarantees `pretty: bool, compact: bool`
exist as params. It cannot guarantee the body calls `resolve_pretty(root, pretty, compact)` —
the macro never sees the body. A method could pass the guard and still do
`self.pretty.set(pretty)` (ignoring `root` and TTY), which breaks the auto-pretty-in-a-real-
terminal behaviour the diagnosis calls out (diagnosis.md:127–129). **The pure guard frame
cannot verify resolution correctness.** This is a genuine limit, not a detail.

Two ways to close it, both honest:

- **Layer 3 backstops it behaviourally:** if mis-wired resolution makes `--pretty` render as
  text, the snapshot test catches it. But a mis-wire that only breaks *TTY auto-detection*
  (correct under explicit `--pretty`, wrong with no flag in a terminal) is invisible to a
  non-TTY CI snapshot. So Layer 3 closes the `--pretty`-explicit case, not the TTY case.

- **Adopt the `CliGlobals` hook (the "minimal hook" the brief permits) to *fix* it, not
  detect it.** This is the recommendation. If the macro auto-wires globals via
  `CliGlobals::set_global_flag` (the sketch in pretty-wiring-audit.md:161–195) and resolution
  is centralised in *one* `display_output`/resolver that always sees `root` + TTY, then:
  defect (a) is **impossible by construction** (no per-method params to forget) **and**
  resolution is correct by construction (one code path, written once). The guard's role then
  *shifts*: Layer 1's invariant becomes "if the impl declares globals, the service must
  `impl CliGlobals`" (still a `compile_error!`, via a trait-bound check the macro can emit),
  and Layers 2–3 remain the (b)/(c) catches.

**Net:** the guard *detects* (a) and (when fixtures hit the path) (c); it does **not** fix
resolution. To *fix* resolution-correctness you need centralisation (`CliGlobals`). The
strongest combined posture is **CliGlobals for the fix + guards as the regression backstop**,
not guards alone.

---

## 5. Migration plan & the "near-zero migration" claim

The brief flags this frame as attractive because it might need near-zero per-command
migration. **That is true for exactly one of the three arms, and it is the weakest arm.**
Verified against the audit's 8 strict-BROKEN + ~46 working + 7 adjacent:

| arm | per-command migration | what it costs | what it buys |
|---|---|---|---|
| **Layer 2/3 tests only** (no `compile_error!`, no marker) | **Near-zero.** Manifest is generated; tests added once. | One fixture corpus; per-command `fixture_args` curation. | *Detect-only*, CI-time, (b) needs the marker, (c) fixture-fragile. Weakest. The "near-zero" claim holds **only here.** |
| **Layer 1 `compile_error!`** | **Not zero — it is the forcing function.** On adoption, the 8 strict-BROKEN commands **fail to compile** until each gets `pretty/compact` params + a `resolve_pretty` call. | The same 8 fixes the diagnosis already prescribes, done all at once because the workspace won't build otherwise. The ~46 working compile unchanged. | (a) becomes a compile error forever. High value, bounded cost (8 methods). |
| **Marker const (`HAS_REAL_PRETTY`)** | **~53 reports** (46 working + 7 adjacent) each set the const to `true`. Mechanical, one line each. | Touch every real-pretty report once. | Enables the (b) check; otherwise (b) is undetectable given the default impl. |
| **`CliGlobals` (the fix, recommended alongside)** | **~6 services** implement the trait; **remove** `pretty/compact` params from ~46 methods + their `resolve_*` calls. | More churn, but mechanical and removes the footgun entirely. | (a) + resolution impossible-by-construction. |

**Verdict on the claim:** *refuted for the strong arms.* The pure-test arm is near-zero but
detect-only and weakest; the `compile_error!` arm deliberately converts the 8 silent bugs
into 8 build breaks (that is the point, not a cost overrun); the marker touches ~53 reports;
`CliGlobals` is the largest churn. "Near-zero" is real only if you accept the weakest variant.

**Recommended sequence (low-risk, incremental):**

1. Land Layer 1 `compile_error!` in server-less + trybuild cases. The workspace stops
   building until the 8 BROKEN methods are wired — fix them in the same change (they are the
   audit's list). Risk: low; blast radius is exactly the 8 methods.
2. Land the inventory manifest emission + the (a)-direction exhaustiveness test (defence in
   depth; cheap once the manifest exists).
3. Add `HAS_REAL_PRETTY` + the (b) check; set the const on the ~53 real-pretty reports.
4. Add the Layer 3 fixture corpus + behavioural test for the commands flagged
   `advertises_pretty && has_real_pretty`; also reach the 7 adjacent unreachable-pretty
   display fns (they fail Layer 3 even though Layer 1 passes — their params exist; their
   bridges ignore pretty).
5. *Recommended, separable:* adopt `CliGlobals` to make (a)+resolution impossible-by-
   construction; at that point Layer 1's invariant flips to "declares globals ⇒ impl
   CliGlobals", and the per-method params disappear.

---

## 6. Honest trade-offs vs the trait-redesign frames

**Where build-guards win:**

- **Low blast radius.** No breaking change to `OutputFormatter` or the `display_with`
  signature. The ~46 working commands compile untouched. A trait redesign that, e.g., makes
  the framework own all rendering changes every consumer at once.
- **Incremental & landable now.** Layer 1 + the 8 fixes is a day's work and immediately kills
  the regression class that actually recurred (stats, subagents, architecture,
  cross_repo_health, 4× rank).
- **(a) is genuinely as strong as a type guarantee** for the specific "advertised-but-unwired"
  shape — a `compile_error!` inside the CLI-generating macro is not a "lint you can ignore."
- **Exhaustiveness via generated manifest** retires the hand-maintained `assert_output_formatter`
  list drift as a side benefit.

**Where build-guards are strictly weaker:**

- **(b) and (c) are CI-time, not compile-time.** They are tests that must be kept green,
  need a fixture corpus, and are *deletable*. A trait redesign that makes pretty-dispatch the
  *only* path to output makes (c) **unrepresentable** — there is no hand-written bridge to
  forget. Guards detect (c); a redesign removes the shape that allows it.
- **(b) is undetectable from types** in this frame; it needs a hand-maintained marker const.
  A redesign where "has pretty" is a real type-level distinction (separate trait /
  associated rendering type) would make (b) a type error. The guard substitutes a maintained
  claim for a type fact.
- **Resolution correctness (root/TTY) is not verifiable by the guard** (§4). Only
  centralisation (`CliGlobals` or a deeper redesign) fixes it; the guard at best detects the
  `--pretty`-explicit symptom via Layer 3.
- **Guard maintenance is ongoing:** fixtures, the marker const, the allow-list for
  near-identical-pretty commands. A redesign pays its cost once, up front, then is
  self-enforcing.

**Bottom line.** This frame's centrepiece — the Layer 1 `compile_error!` for defect (a) — is
the single highest-leverage, lowest-risk move on the table and should land regardless of
which frame wins overall: it makes the exact bug that recurred a build break, with bounded
migration. But the frame is honestly *partial*: (b) and (c) degrade to maintained CI tests
with real reliability limits, and resolution-correctness is outside its reach. If the goal is
"all three impossible by construction," guards alone do not get there — pair the (a) guard
with `CliGlobals` (for (a)+resolution) and accept (b)/(c) as CI-enforced, or choose a trait
redesign that removes the bridge and the default-impl ambiguity that make (c) and (b)
representable in the first place.
