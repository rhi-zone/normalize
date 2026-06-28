# Judge — migration cost / blast radius / sequencing / architecture fit

**Date:** 2026-06-28
**Role:** adversarial judge. Attack the four designs on MIGRATION COST, BLAST RADIUS,
MAINTENANCE BURDEN, CROSS-REPO SEQUENCING, ARCHITECTURE FIT. Challenge every "net deletion"
and "near-zero" claim with a real count.
**Companions:** `design-A-subtract.md`, `design-B-type-property.md`, `design-C-invert.md`,
`design-D-build-guard.md`, `pretty-wiring-audit.md`, `diagnosis.md`.

---

## 0. Ground-truth counts (verified against the tree, not the digests)

The designs throw around "~16 real pretty," "~80 types," "~46 working," "net deletion,"
"near-zero." Measured:

| fact | command | value |
|---|---|---|
| `fn format_pretty` overrides | `grep -rc "fn format_pretty"` | **65** (across 54 files) |
| `impl OutputFormatter` blocks | `grep -c "OutputFormatter for"` | **164** |
| `display_with = "..."` attrs | `grep -c` | **161** total |
| of which `display_output` (the text/pretty bridge) | | **93** |
| of which **bespoke** custom display fns | | **~68** distinct (`display_translate`, `display_trace`, `display_ast`, `display_view`, `display_call_graph`, `display_query`, `display_schema`, `display_history`, `display_graph`, `display_imports`, `display_lint_*`, `display_measure`, `display_check`, `display_chunked`, `display_provenance`, `display_cfg`, `display_set`, `display_validate`, `display_add/update/remove`, …) |
| services with `pretty: Cell<bool>` + `global=[pretty,compact]` | | **12** |
| `assert_output_formatter::<T>()` hand-listed entries | | **148**, with a live drift marker `// Task 4: missing entries` at `output.rs:245` |
| `process::exit` in the service layer | | **3**, all in `sessions.rs` (`by_repo`, `group_by`) |
| crates published | `45 dirs − 1 publish=false` | **~44** |
| intra-workspace dep style | `crates/normalize/Cargo.toml:52` | **`path = ".." , version = "0.3.2"`** (path present!) |
| cross-repo dep on server-less | `Cargo.toml:123` | **`version = "0.4.9"`, no path** |

**Two of these immediately falsify headline claims:**

- **B's "~16 real pretty report types" is off by ~4×.** There are **65** `format_pretty`
  overrides. Even discounting the trivial delegators (`normalize-native-rules/ratchet.rs:23`,
  `budget.rs:23` just forward to `self.0.format_pretty()`), the real-pretty population is
  well over 50, matching the audit's own "~46 working + 8 broken + 7 adjacent ≈ 61", not 16.
  B's §5 cost table is built on the wrong denominator.

- **The "delete all `display_with`" claim (A/B/C) is wrong by ~68.** Only the **93**
  `display_output` bridges (plus a handful of pretty-branching siblings like `display_analyze`,
  `display_move`) collapse. The other **~68** bespoke display fns render *neither* a text/pretty
  split — they are custom one-shot renderers (`translate`, `view trace`, `syntax ast`, call
  graphs, schema dumps). They have no pretty to merge and **must survive** every redesign.
  B §6 admits this in passing; A and C gloss it in their "delete every `display_with`" steps.

---

## 1. Cross-repo sequencing — the intermediate state per design

The cut that matters: **server-less is a version-only cross-repo dep (no path).** Any design
that changes the `#[cli]` macro must (1) land in `/home/me/git/rhizone/server-less`, (2)
publish a new version, (3) bump `server-less = "0.5.0"` in normalize, (4) then migrate. There
is **no atomic cross-repo commit**. By contrast, `normalize-output` is intra-workspace with a
`path` dep, so a trait change there *can* be one local commit (B's flag day is locally
compilable — see below).

**All four designs need a server-less publish+bump.** Including D: its centrepiece Layer 1
`compile_error!` lives in `server-less-macros`, and its inventory emission is a macro change
too. D's "landable now, low blast radius" is true **only for its weakest arm** (Layer 2/3
tests with no macro change). The strong arm pays the same sequencing tax as A/B/C.

| design | server-less change | additive? | normalize compiles between bump and migration? | incremental per-service migration? |
|---|---|---|---|---|
| **A** | new `CliRender` trait + `RenderMode` + rewrite text branch of `gen_value_display`; keep `display_with`/`Display` branches | **yes (additive)** | yes | **yes** — convert service-by-service after the bump; old `display_with` path coexists |
| **B** | `Render`/`PrettyProbe`/`PrettyPolicy` probes + advertise-`if` | **yes (additive)** in server-less; but **breaking in `normalize-output`** | only after the trait split lands | **no — flag day** (trait split + all consumers in one commit) |
| **C** | `CliTextFlags`/`CliTextRender<T>`/`AsConfigRoot` + per-impl `render` flag + `#[param(config_root)]` | **yes (additive)** | yes | **per-impl, not per-method** — and blocked on impl homogeneity (§3) |
| **D** | `compile_error!` guard + `inventory::submit!` emission | **yes (additive)** | yes (guard only fires on opt-in `global=[pretty,compact]` impls) | **yes** — but the guard is a *forcing function*: the 8 BROKEN methods stop compiling the moment the bump lands |

**Sequencing-pain ranking (worst → best): B ≫ C > A ≈ D.**

- **B is worst.** Even though `normalize-output` has a `path` dep (so the split *compiles*
  locally in one commit), that one commit is a breaking change to a **published core trait**
  consumed by ~44 crates. At release it forces a coordinated major bump of `normalize-output`
  **plus every feature crate that overrides `format_pretty`** (see §2). "Do it in one commit,
  lean on `cargo check`" is feasible to *compile* but is a genuine flag day: no partial
  landing, and the published-crate version cascade is the largest of any design.
- **A and D additive in server-less and incremental afterward** are the gentlest. A can walk
  service-by-service because it preserves the legacy branches; D's guard only bites the 8
  already-broken methods.

---

## 2. Blast radius — B's "near-zero outside the main crate" is FALSE

B §5 asserts: *"Surveyed feature crates … have no real pretty … they implement only
`format_text`, so they need no source change and only recompile. Blast radius outside the main
crate is therefore near-zero."*

Refuted by grep. **Real `format_pretty` overrides exist in at least five published feature
crates:**

- `normalize-context/src/lib.rs:80` — real (renders `self.blocks`).
- `normalize-graph/src/lib.rs:196` — real (`format_modules_text(true)` / `format_flat_text(true)`).
- `normalize-rules/src/service.rs:227, :325` and `runner.rs:334` — real (`nu_ansi_term` color).
- `normalize-native-rules/src/ratchet.rs:23`, `budget.rs:23` — delegating overrides.
- `normalize-session-analysis/src/lib.rs:1238` — the trigger case, real.

Under B every one of these must move from `OutputFormatter::format_pretty` into
`impl PrettyFormat` — a **source change in five-plus independently-published crates**, all
gated on the `normalize-output` major bump, none of which can publish until the trait split is
released. B's blast-radius claim is the single biggest inaccuracy across the four docs.

**A is even wider but more honest about it.** A replaces `OutputFormatter`'s two methods with
a single `CliRender::render(mode)`. That is not 80 types — it is **all 164 `OutputFormatter`
impls**, including the ~100 text-only reports that never had pretty (their `format_text`
becomes the `Plain` arm of `render`). A's §5 "~80 report types" undercounts by ~half: every
report in the workspace changes trait, in every published crate. A's blast radius is the
*largest*, but A at least frames it as "large but shallow and mechanical."

**C and D keep `OutputFormatter` intact** → the published-trait blast radius is the smallest.
C touches only the service layer + the move of pretty resolution into one blanket impl; the
164 report impls are untouched. D touches only the macro + a per-report `HAS_REAL_PRETTY`
const.

---

## 3. The touch-point trace through three real services (sessions, analyze, rank)

Honest per-method accounting, including the methods that **don't** fit the uniform pattern.

### Methods that break the uniform pattern (friction every digest glossed)

1. **`sessions stats` `by_repo` / `group_by` print-and-exit** (`sessions.rs:281–323`).
   `by_repo` calls `self.display_output(&report)` then `process::exit(0)`; `group_by` calls
   `show_stats_grouped(...)` then `process::exit(exit_code)`.
   - **A** addresses it (step 7: return the report; add a `GroupedStatsReport` wrapper).
   - **C** addresses it (§6: fold into the return type).
   - **D** keeps it working unchanged (it keeps `display_output`).
   - **B never mentions it.** B deletes `display_output` (§1.3) but leaves the call site at
     `sessions.rs:295` dangling — **B will not compile** at that line, and the doc has no plan
     for it. Concrete migration hole.

2. **Mixed-renderer impl blocks.** The main `NormalizeService` `#[cli]` impl
   (`mod.rs:251`, under `global=[pretty,compact]` at :240) contains **both** `display_output`
   methods **and** bespoke renderers (`display_translate` at :643, plus `display_trace`,
   `display_history`, `display_graph`, `display_import_path`, `display_dependents`).
   - **C is broken by this.** C's `render` flag is **per-impl** and "switches *every* method in
     that impl into renderer mode," requiring homogeneity (§5 "split a service if it mixes").
     C then claims "in practice the audited rendering services are already homogeneous" —
     **false for `NormalizeService`**, which mixes report-rendering and custom renderers under
     one impl. C's migration silently requires splitting the main service, a refactor it never
     scopes.
   - **A handles it cleanly.** A keys off *presence of `display_with`*, not the impl: a method
     with no attribute uses `CliRender`; `display_translate` keeps its `display_with`. Mixed
     impls are fine.
   - **B handles it** (per-method, type-driven) but inherits the dangling-`display_output`
     problem from (1).
   - **D handles it** (leaves everything; guard only checks param presence).

3. **`Vec`-returning / multi-report methods** (8 `Result<Vec<…>>` in the service layer).
   A/B/C dispatch on the report type; a `Vec<T>` return goes through the macro's existing
   `Vec`/`Map` printing branch, **not** `CliRender`/`PrettyFormat`/renderer-mode — so those
   commands silently keep the *old* path and gain no pretty. None of A/B/C scope what happens
   to `Vec`-returning commands that want pretty. D is unaffected (it doesn't change dispatch).

4. **Unit / `"Done"` returns** (macro special-case, `cli.rs:1964`). C §5 notes `render` impls
   must contain only report-or-unit methods or fail to resolve `CliTextRender<T>`; combined
   with (2) this tightens the homogeneity squeeze on real services.

### Net touch-point verdict per design

| design | service-layer edits | report-layer edits | new types / refactors | **honest cost** |
|---|---|---|---|---|
| **A** | 12 services (drop Cell/params/`display_output`), `render_root` marker on every rooted method (~50), `render_mode` hook ×12 | **all 164** `OutputFormatter`→`CliRender` | `GroupedStatsReport`; `by_repo`/`group_by` refactor; trybuild for new branch | **HIGH** (~164 reports + 12 services + exit refactor) |
| **B** | 12 services (drop Cell/params/`global=[]`/`display_output`) | **~65** move to `impl PrettyFormat` across **6 published crates** | exit-path call sites (**unscoped**); specialization regression test | **HIGH** (~65 across 6 crates, + flag day, + unaddressed exit paths) |
| **C** | 12 services + **split mixed services** + `NormalizeRendered` tags + blanket impl + `AsConfigRoot` type coverage | 0 report-trait changes | exit refactor; **service splits** (unscoped); `#[param(config_root)]` on positional-root commands | **HIGH** (service splits are the hidden tax) |
| **D** | 8 BROKEN methods get params + `resolve_pretty`; (recommended) `CliGlobals` on ~6 services | **~53–65** reports set `HAS_REAL_PRETTY=true` | fixture corpus + per-command `fixture_args`; trybuild | **MED** for Layer 1 + 8 fixes; **HIGH** if you want (b)/(c) too (marker on ~60 reports + fixtures) |

No design is "net deletion" in the clean sense advertised. A *adds* a marker to ~50 methods
and a new report type. B *adds* `impl PrettyFormat` blocks (a move, not a deletion) across six
crates. C *adds* service splits + marker tags + a type-coverage trait. D *adds* a maintained
const to ~60 reports plus a fixture corpus. The deletions (params, `self.pretty.set`) are real
but are dwarfed by the trait/marker churn.

---

## 4. Architecture fit ("service returns typed data, macro renders")

- **C is the most explicitly API-first** in intent — the method returns pure data and writes
  *zero* plumbing; resolution lives in one blanket impl. **But** C leaves `OutputFormatter`'s
  defaulted `format_pretty` in place, so the *trait* still carries the "pretty == text by
  default" ambiguity that is the root of (b). C buys API-first at the service layer while
  leaving the data-model footgun in the trait. **Grade: A−.**
- **B encodes the cleanest data fact** — `impl PrettyFormat` *is* "this type has pretty,"
  non-defaulted, honoring the no-stub rule. This is "prefer data over code at a seam" done
  right: capability is a type fact, advertising and dispatch are *projections* of it
  (library-first / projection-from-one-definition). The cost is the inherent-vs-trait
  specialization "trick" (B §6) — clever, stable-but-subtle, and B itself flags advertising as
  the fragile half. **Grade: A.**
- **A restores symmetry with the machine-format path** (one value, macro-resolved mode,
  macro-driven render) — architecturally satisfying and it kills the `Cell` (aligns with the
  "config flows in via params, not out via globals" rule). But `render(mode) -> String` folds
  two render methods into one mode-branching method that the *report* owns — a half-step back
  from "typed data, macro renders" toward "report renders itself." It also passes up the
  `Write`-based streaming renderer (A admits). **Grade: A−.**
- **D does not move toward API-first at all** — it keeps the `Cell`, `display_with`, and manual
  `resolve_pretty`, then bolts guards on. D's own §4 concedes the guard *cannot fix*
  resolution correctness and recommends adopting `CliGlobals` (a redesign) to actually fix it.
  D is a *regression backstop*, not an architecture. **Grade: C.**

**None distorts data shape**, with one shared positive: A and C both force the `by_repo`/
`group_by` `process::exit` anomaly back into the return-data contract — a genuine
architecture improvement that B and D leave as-is (B by omission, D by design).

---

## 5. Maintenance burden of D specifically — is the guard self-maintaining?

Mixed. Honest breakdown:

- **Layer 1 (`compile_error!` for (a))** is genuinely self-maintaining — it is macro output,
  not a list. Non-deletable without editing `server-less-macros` (and the trybuild case goes
  red if someone does). **This is the one part of D that doesn't drift.**
- **The inventory manifest** (`inventory::submit!` per method) is macro-emitted → the *data*
  can't drift. Real upgrade over the status quo.
- **`HAS_REAL_PRETTY` const** is exactly the hand-maintained-list anti-pattern CLAUDE.md warns
  about. It is `false` by default on ~60 reports; a dev who writes a real `format_pretty` and
  forgets the const gets a **false positive** (loud, but wrong). D §2 admits "it trades
  'forget to wire the flag' for 'forget to set the const'." This **is** the next
  `assert_output_formatter` — and that list has *already drifted* (`output.rs:245`
  `// Task 4: missing entries`, 148 hand-listed entries). D would add a second const to the
  same drift surface.
- **Layer 3 fixture corpus + per-command `fixture_args`** is a maintained artifact whose
  fragility D names itself (empty-report false-equality; per-command curation; allow-list for
  near-identical-pretty commands). This is the least self-maintaining piece.

**Verdict:** D's *centrepiece* (Layer 1) is self-maintaining; its (b)/(c) layers become the
next stale lists. D is honest about this — it is not over-claiming — but it means D's full
posture carries permanent maintenance the redesigns mostly retire.

---

## 6. The residual gap each design genuinely CANNOT close

Every design leaves at least one gap that still needs a CI test after it ships.

| design | what it makes impossible-by-construction | **residual gap (still needs CI)** |
|---|---|---|
| **A** | (a) silent no-op, (c) dead dispatch | **(b):** the two arms of `render(mode)` can *coincide by accident*; the type system can't tell "intended to differ." Needs distinctness test + `has_pretty` tag. Plus: `render_root` marker can be forgotten → project config ignored for non-cwd `--root` (lint-only). |
| **B** | (a), (b)-advertising (gated on `PrettyFormat`), (c) | **(b)-content:** a `PrettyFormat` impl whose `format_pretty` body *equals* `format_text`. Type system can't catch body equality. Needs the same distinctness test. Plus: advertising-probe relies on inherent-const priority — needs a `rustc`-version regression test. |
| **C** | (a), (c) | **(b):** `OutputFormatter`'s defaulted `format_pretty` is **untouched**, so "report didn't differentiate, flag is honest identity" stays representable. C explicitly downgrades (b) to a non-defect + optional lint. Largest residual of the three redesigns. Plus config-root mis-selection by name convention. |
| **D** | (a) only (Layer 1) | **(b) AND (c)** both stay CI-time/deletable; **resolution correctness (root/TTY)** is unverifiable by the guard (D §4). Three residual gaps — the most of any design. |

**The universal residual (no design closes it):** *"pretty output is byte-identical to text
output despite intent."* A (coincident arms), B (equal bodies), C (default impl), D (explicit)
all leave it. **A pretty≠text behavioural distinctness test — run the command both ways, strip
ANSI, assert a structural difference on a populated fixture — is required REGARDLESS of which
design wins.** That is exactly D's Layer 3; the redesigns do not eliminate the need for it,
they only shrink the set it must cover.

---

## 7. Bottom line / recommendation

- **Best raw cost/guarantee ratio for the bug that actually recurred:** **D's Layer 1
  `compile_error!`** — bounded to the 8 BROKEN methods, non-deletable, and it turns the exact
  recurring defect (a) into a build break. It does *not* fix resolution or (b)/(c), and its
  marker/fixture layers become maintained lists.
- **Best architecture + strongest type guarantee:** **B**, but it carries the worst sequencing
  (flag day) and worst published-crate blast (≥6 crates, ~65 reports — not the 16 it claims),
  and it has an unaddressed compile hole at the `sessions stats` exit paths.
- **Cleanest API-first intent:** **C**, undercut by the per-impl homogeneity requirement that
  breaks on the real mixed `NormalizeService` impl — a service split it never scopes.
- **Widest but shallowest, and handles mixed impls best:** **A** — but it rewrites all 164
  report impls (not 80) and is a breaking change to the core `OutputFormatter` trait.

**Synthesis the bake-off should consider:** the marginal guarantee A/B/C add *over* "D Layer 1
(for (a)) + `CliGlobals` (for resolution)" is essentially **(c)-by-construction** — and *none*
of them close (b), which needs the Layer-3 distinctness test either way. That is a large
mechanical migration (65–164 reports across published crates) to buy (c)-by-construction plus
cosmetic deletion. The honest framing for the decision-maker: **(a) and resolution are the
defects that bit users; both are closable with D-Layer-1 + CliGlobals at MED cost and no
published-trait break.** Spend the HIGH redesign budget only if (c)-by-construction is judged
worth a core-trait major version across ~44 crates.
