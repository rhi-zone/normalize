# Inferred Opinionation: Guess Configuration / Taste / Consensus for Free

Design notes for inferring canonical forms from the codebase itself — opinionation
without hand-written rules.

## Status

Backlog — not yet started. This is a *future* goal for normalize: today nothing in
the toolchain collapses the valid-program space exponentially (see below). This doc
records the design and, importantly, the **prerequisite architecture** it presupposes.

---

## The Overarching Goal: Collapse the Valid-Program Space

The motivating problem: a large LLM-generated codebase (tens of thousands of commits)
accumulates bugs faster than any per-commit review or test-writing budget can catch
them. Generating the code a second time, per-commit agentic review, and bigger CI
matrices are all prohibitively expensive at that scale.

Reframed information-theoretically, the target is the gap **valid ∖ correct**:

- The space of *valid* programs (parses, type-checks, runs) is enormous: with `N`
  decision points and `k` choices each, roughly `k^N`.
- The space of *correct* programs is a tiny subset.
- **Bugs live in `valid ∖ correct`** — programs the tooling accepts but that are
  wrong.

The game is to **collapse the valid space so it hugs `correct` from above** — shrink
`k^N` toward the correct set without ever dropping *below* it. "Below" matters: if
canonicalization forbids a legitimately-needed variant you have banned a correct
program (collapse below the true entropy `H` of the design). So the constraint is:
maximize collapse subject to never excluding a genuinely-needed form. The escape hatch
for "needed but non-canonical" is an **accounted deviation** (a note), which is the
boundary normalize shares with the decision-stream / decision-engine work.

This splits cleanly along **decidability**:

- **normalize** collapses the *decidable* bulk. Canonicalization — reducing
  semantically-equivalent programs to a normal form — is pure rewriting, no halting
  oracle required. Cheap, bulk, and **retroactive**: because it is behavior-preserving
  and decidable, you *can* normalize a 20k-commit backlog (you cannot semantically
  re-derive it).
- **crescent** (the language / typechecker) collapses the *semantic* residual — the
  hard, undecidable tail, forward-going.

A bug, in the combined frame, is **an unaccounted deviation from the enforced norm**:
conform to canonical → no note needed; deviate *with* a note → an accounted essential
decision; deviate *without* a note → that is the bug. Bugs become the *residue* of
canonicalization + accounting, made to stick out against a canonical background
("something unexpected is a signal," mechanized) rather than hunted as needles.

---

## Exponential vs Linear Collapse

The critical distinction — and the reason today's normalize does **not** yet do this:

- **Linear collapse** removes a constant *number* of states. A linter that flags 50
  bad instances, or bans a fixed list of patterns, subtracts a fixed count from `k^N`.
  It barely dents the space. **All of normalize's current diagnostics (flag, measure,
  hold) are linear** — they enumerate violations; they do not reduce dimensionality.
- **Exponential collapse** constrains a constant *fraction* — a whole decision-class,
  *everywhere*. Reducing the per-node branching factor from `k` to `m` at every one of
  the `N` points turns `k^N` into `m^N`. That multiplicative effect is the only thing
  that meaningfully shrinks the space.

The mechanism behind exponential collapse is **representational, not diagnostic**: you
make non-canonical forms *unrepresentable* (change the grammar / type system / the set
of forms the engine will emit), rather than flagging them after the fact. A linter
removes points (linear); a restrictive grammar or type system removes whole dimensions
(exponential). So normalize's future exponential-collapse capability must be
**constructive/rewriting** (conform every construct to canonical form so the codebase
converges to a single representative per equivalence class), not merely a new family of
warnings.

---

## Prerequisites / Required Architecture

**Inferred opinionation presupposes infrastructure that can enforce AND measure
arbitrary constraints — including non-numeric / categorical ones — globally across a
codebase.** This is the precondition, and it is currently only partially present. It
must be a first-class build-out, not an afterthought, or the rest of the pipeline has
nothing to stand on.

What "enforce and measure arbitrary constraints globally" requires:

1. **A global view.** Conventions are corpus-level facts ("the consensus form for
   this construct, across the whole tree"), not file-local. Measuring and enforcing
   them needs whole-codebase indexing of instances per decision-class.
2. **Enforcement (constructive).** Conform every instance to its canonical form via
   the rewrite/refactor/edit engine — the representational collapse above.
3. **Measurement (a held quantity).** A way to *measure* conformance globally and
   hold the line so it cannot regress. **This is where the gap bites:** see the
   ratchet open question below — today's measurement substrate is numeric-scalar
   only, which covers *counts* but not genuinely categorical/set-valued measures.

Until this measure-and-enforce-arbitrary-constraints substrate exists, inferred
opinionation cannot be deployed; building it is the first work item.

---

## Configuration by Consensus

Today, canonical forms must be supplied externally — a reference file, an eglint
config, a manually curated rule set. "Format one file to format them all" is already a
step forward, but a codebase is **not a singular example**. It is a *corpus*. The
distribution of choices already encoded in the codebase IS the configuration — we just
haven't read it.

For each decision-class (formatting, control-flow style, expression ordering,
delimiter choices, etc.), the codebase is a distribution of choices. The convention is
the **consensus** over all instances. The goal: infer style conventions automatically
from the corpus via heuristics, with no hand-written rules — opinionation guessed for
free.

(This is explicitly **not** self-similarity / format-by-example, and **not** an eglint
clone — both treat one file/example as the spec. The unit here is the whole-corpus
distribution.)

**Scope: style / formatting / control-flow only.** Style has no "correct," only
"consistent" — consensus = consistency, so you cannot enshrine a "wrong" style.

**NOT for semantic / correctness canonicalization.** There the modal pattern can be
the modal *bug* — enshrining LLM slop. The slop-trap applies to *semantic*
canonicalization; it does not apply to style. Semantic consensus is unsafe.

### Shallow vs Deep Opinionation

A load-bearing distinction within style:

- **Shallow** (gofmt/prettier): collapses *layout* — whitespace, line breaks.
  Cosmetic, near-zero bug impact, but it is the purest accidental complexity:
  representational bits carrying *zero* semantic information. Canonicalizing them
  lowers representation entropy, which *raises mutual information* — every downstream
  oracle gets more grip (the typechecker has more to bite, diffs become meaningful
  instead of style-noise, anomalies become legible). normalize as the entropy-floor
  enforcer.
- **Deep**: one blessed *pattern* per task ("there is one way to handle this error /
  acquire this capability / construct this"). Because the blessed pattern is the
  *safe* pattern, conforming = correct, and **valid hugs correct** along every axis
  canonicalized. This is where `valid ∖ correct` actually shrinks — the real
  bug-collapse, not the cosmetic one.

---

## Distribution Shape, Not Binary

The per-class vote is a distribution over *k* forms, not a binary. Let **`m` = the
number of significant modes** in that distribution (k-way, not two-way). The `m` modes
are the **blessed set**; the off-peak **tail** is the set of collapse targets. (This is
the `k → m` framing: collapse the `k` observed forms to the `m` legitimate ones.)

| Shape | m | Action |
|-------|---|--------|
| Sharp unimodal | 1 | Collapse all instances to the one form |
| Multimodal | #peaks | Legitimate forms — keep all; collapse only the off-peak tail |
| Flat | ? | Free decision (no convention) or chaos — see below |

The flat case is resolved by the decision-tree step.

---

## Normalization Without Confluence

A common objection: "there is no *unique* normal form for control flow, so you can't
normalize it." This is wrong, and the wrongness is the whole point:

> **"No unique normal form" ≠ "no normal form."**

A rewrite system that is **semantics-preserving** and **terminating** reaches normal
forms even if it is **non-confluent** (different reduction orders can land on different
irreducible terms). You never need to prove confluence. And the non-confluence is not a
defect to be engineered away — **the non-confluence IS the decisions.** Where two
distinct normal forms both survive, that residual choice is precisely a genuine
decision (taste / convention / a real free choice), which is exactly what the decision
stream is supposed to capture. normalize does not need to produce a single canonical
output to be useful.

**Control flow is the #1 decision-class** to target: highest entropy × highest
bug-density, the most opinionated decisions with the most structural variation — and
normalize's CFG/scope model already supplies the features to explain that variation.

(How much of the state space is normalizable is an open empirical question — assume
nowhere near 100%, but plausibly nowhere near 0%. The more interesting question than
"how much" is "which *kinds* of properties" can be normalized away.)

---

## Decision-Tree Formalization

The crux: learn a **per-decision-class decision tree**.

- **Features = structural context**: parent construct, scope type, nesting depth,
  sibling shape, CFG position, facts. normalize's own AST / CFG / scope / facts model
  is the feature source — no external annotation needed.
- **Label = the form chosen** at each site.
- **Internal nodes = context conditions; leaves = conditional conventions.**
- **Leaf purity = strictness** of the rule at that context.

**Information gain (entropy reduction)** is the mechanism — the same entropy meter from
the Shannon framing, made into an algorithm. The tree decomposes variance:

```
context-explained variance  →  conditional rule  →  collapse
residual leaf impurity       →  genuine decision  →  surface (decision stream)
```

This resolves the flat-marginal ambiguity cleanly:

- Flat marginal **but pure under context** → a conditional rule (e.g. "short-form when
  inside a closure body; long-form at top-level").
- Flat **and stays impure at every leaf** → a real free decision — the variance no
  structural context can predict — surfaced to the decision stream for human
  resolution. That is precisely the set of choices *genuinely underdetermined* by the
  codebase.

### MDL Pruning Is Mandatory

Without it, the tree memorizes slop as spurious rules. **Accept a split only if it
*compresses*** — the information gain must pay the description-length cost of the added
node. A split is "real" iff it shortens the total description (data given model + model
itself). MDL pruning is the false-oracle / overfit filter, applied here as tree
regularization. This is not optional polish; it is the thing that distinguishes an
inferred *convention* from memorized noise.

---

## Output Compiles to the Rule Engine

A decision tree is interpretable: each root-to-leaf path is a conjunction of
conditions, which is a stateable rule. The learned tree emits directly as `normalize`
fact/syntax rules, enforced and conformed via the refactor/edit engine.

**Conventions are learned AND legible, not hand-written, not a black box.**

The pipeline:

```
corpus instances  →  normalize-code-similarity (normalized AST hashing, bucketing)
                  →  decision-tree learner (per decision-class, MDL-pruned)
                  →  rule paths  →  normalize-rules / normalize-syntax-rules
                  →  normalize-refactor / normalize-edit (conform)
                  →  normalize-ratchet (hold the line)   ← SEE OPEN QUESTION
```

Feature extraction draws on `normalize-cfg`, `normalize-scope`, and the facts model —
the same structural information already computed for other analyses.

### Open Question: Does `normalize-ratchet` Support the "Hold" Step?

The "ratchet hold" terminal step is **not** an established capability — do not assume
it works. Investigation of `normalize-ratchet` / `normalize-metrics` (code-grounded)
found:

- The measure type is **`f64` — numeric scalar only**. The pipeline is locked to it:
  `Metric::measure_all -> Vec<(String, f64)>` (`normalize-metrics/src/lib.rs`),
  `BaselineEntry.value: f64` (`normalize-ratchet/src/baseline.rs`),
  `compute_aggregate(... ) -> Option<f64>`. There is no enum/union for booleans, sets,
  or categorical buckets.
- The metric *registry* is open/extensible (`MetricFactory`,
  `RatchetService::with_factory` accept any `impl Metric`) — but only for measures
  expressible as `f64`.
- "Regression" is a monotone scalar comparison (`current > baseline` for
  `higher_is_worse`).

**Verdict:** ratchet *can* hold the line **today** on a *count* — "number of
non-conforming instances in decision-class C must not increase" — because that is an
`f64` per address; implement a `Metric` returning the count, leave `higher_is_worse`
true, and the existing comparison works. **New work is required only for genuinely
non-numeric measures**: if a constraint produces a categorical/boolean/set value rather
than a count, `Metric::measure_all -> Vec<(String, f64)>` cannot represent it, and the
trait signature needs a new value type. So the prerequisite-architecture gap is narrow
but real: counts are covered; categorical global measures are not.

---

## Honest Bounds

1. **Ceiling is the feature model.** Only as good as the structural context exposed by
   normalize's AST/CFG/scope. A convention with no extractable feature to explain it
   will masquerade as a "free decision" (a false high-entropy leaf). normalize's
   structural model is the hard ceiling on what can be inferred.

2. **MDL / pruning is mandatory.** Without it the tree memorizes slop as spurious
   rules (see above).

3. **Scoped to style.** Semantic canonicalization is excluded — the modal pattern can
   be the modal defect.

4. **Over-opinionation collapses below `H`.** If canonical form forbids a
   legitimately-needed variant you have banned a correct program. normalize therefore
   *needs* the accounted-deviation escape hatch (the note) — the same accounting system
   the decision-engine uses; normalize is its decidable half.

5. **It does not kill the semantic tail.** Canonical-but-wrong is possible (right
   pattern, wrong logic). normalize annihilates the structural/stylistic bulk and makes
   the tail *legible* — it does not decide it. That residue is the oracle's / crescent's,
   now on a small high-contrast surface instead of 200k chaotic lines.

---

## See Also

- `normalize-code-similarity` — normalized AST hashing → instance bucketing per
  decision-class.
- `normalize-rules`, `normalize-syntax-rules`, `normalize-facts-rules-*` — compile
  target for learned rule paths.
- `normalize-cfg`, `normalize-scope` — feature source for structural context.
- `normalize-refactor`, `normalize-edit` — conform engine.
- `normalize-ratchet`, `normalize-metrics` — enforcement / hold-the-line (numeric-only
  today; see the open question above).
- Companion essays in the rhi docs repo (the broader Shannon → decision-stream →
  crescent arc this design sits inside):
  `docs/decision-stream.md` @1d1ad10,
  `docs/deterministic-simulation-testing.md` @90a6350.
