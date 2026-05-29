# Ad-Hoc Dispatch

**Status:** Stable reference — detection design tenet.

A recurring architecture smell across the normalize ecosystem. Understanding what it is and how to detect it informs both code review and the design of a candidate lint rule. For the planned rule and normalize's own self-violations, see `TODO.md` (search "drifted-dispatch-tables").

---

## The Thesis: Complexity Is Not the Smell; the Axis of Dispatch Is

A useful natural experiment: crescent's Lua typechecker has three generations.

- `static/` (v3) — the original typechecker, suspected ad-hoc
- `static-v4/` — a principled SimpleSub/MLstruct rewrite
- `static-v5/` — formal operational-semantics rewrite

`static/solve.lua` is 4231 lines. `static-v5/op_sem.lua` is 3010 lines with 51 named rule-functions. Both are large. But v5 is principled and v3 is ad-hoc.

**Raw size, LOC, and handler count are not the discriminator.** A size-based metric would correctly flag v3 but also misclassify v5. The discriminator is structural: *what is the dispatch keyed on?*

---

## The Discriminator: Four Signals

**1. Dispatch keyed on a feature/name enum vs. dispatch keyed on data structure or type shape.**

Ad-hoc dispatch: one bespoke handler per case, keyed on a closed name — `C_ARITH`, `C_INDEX`, `"sin"`, `"docx"`, `EngineOrigin::Flash`. The handler for each case is its own island of logic.

Principled-but-complex dispatch: cases reduce to existing constructors or a lattice law. Adding a new input type is a matter of composing existing rules, not writing a new handler.

**2. New responsibility ⇒ new code in the engine vs. new data in a registry.**

In crescent v3, arithmetic type-checking lives in a 91-line `solve_arith` function inside the engine. In v4, arithmetic is typed via metamethod records in a configuration table — zero new engine code. When adding a new operator in v3, you modify the engine. In v4, you add a row to a table.

**3. Escape hatches the codebase itself calls "provisional."**

v3's `$`-intrinsic table — an ad-hoc special-case mechanism layered on top of the already-ad-hoc constraint solver — was documented in crescent's own CLAUDE.md as a design violation that required a full rewrite to undo. When a system needs an escape hatch for its own escape hatches, that is a reliable signal.

**4. Loud rejection of the unhandled vs. silent bolt-on.**

Principled systems have a clear "this case is not handled" path — v4's `index.lua` explicitly refuses a one-off pending-index queue rather than bolting it on. Ad-hoc systems accumulate special cases silently until someone notices the rot.

**5. Provenance: spec-traceable vs. bug-traceable.**

v5's `rule_<label>` functions map to entries in a formal semantics document. v3's handlers trace to bug reports and edge cases discovered at runtime. When every handler has a corresponding spec entry, the system is principled. When handlers trace to "we needed to handle X so we added it," the system is ad-hoc.

---

## The Recurring Structural Signature

Across the ecosystem, ad-hoc dispatch manifests as one recurring structural pattern:

> **N parallel dispatch tables keyed on the same closed name-set, where one registry/trait/visitor belongs.**

The strongest tell is **drift**: the parallel tables disagree. The set-difference of their keys identifies cases that are handled in some tables but not others — which is a mechanical fact derivable from source code alone.

This pattern recurred across every codebase in the ecosystem investigation (see Appendix below).

---

## Lint Tiering: What Is Deterministically Detectable

### Deterministic / AST-queryable

These signals can be computed mechanically from source alone:

- **N parallel dispatch tables over the same name-set** — the core structural pattern
- **Drift between parallel tables** (set-difference of key-sets) — the highest-value signal, fully mechanical
- **Large match/if-chain on a kind or name field with oversized individual handlers** — size-weighted dispatch width
- **Closed if-chain on an interned name** — string-keyed dispatch that should be a registry lookup
- **`grammar_name ==` in a language-agnostic crate** — hardcoded language dispatch that violates the Language trait pattern (this one is already in normalize's CLAUDE.md as a hard constraint)

### Requires Judgment (LLM or human review)

These signals require understanding intent, not just structure:

- Whether a bespoke handler is *necessary* or could be expressed as data
- Whether an escape hatch *should* be general (was the intent to go back and generalize it?)
- Whether an unhandled case was explicitly declined or silently omitted
- Whether the code follows its cited specification (requires reading both)

---

## Feasibility: normalize Can Already Extract the Signal

A spike confirmed that normalize's existing extraction infrastructure is sufficient to detect the structural pattern. `normalize syntax query` can pull pattern-scoped string-literal match/switch keys with their positions. Drift computation is plain set-difference on top.

The spike reproduced both known drift cases mechanically:

- **wick**: `inverse_lerp` and `remap` absent from cuda/hip backends; `asinh`, `cbrt`, `fma`, `hypot`, `copysign` absent from CPU backends — all detected by set-difference over the 11 parallel `*_func_name` tables in `wick-scalar`.
- **marinada**: 46 `Expr` operation kinds typed but not evaluated — detected by set-difference between the `typecheck.ts` switch and the `evaluate.ts` switch.

**To become a real rule**, three pieces of rule-side logic are needed (the extraction itself is sufficient):

1. **Per-function scoping by row-range** — to isolate each individual dispatch table rather than treating a whole file as one bag of keys.
2. **Jaccard key-set clustering** — to identify which dispatch tables are drawing from the same "roster" and should be compared, vs. unrelated dispatchers in the same file. Without this, the signal is buried in false positives.
3. **Set-difference + min-table-size threshold + a baseline/allowlist** — to distinguish genuine drift from intentional asymmetry. Some tables are deliberately asymmetric.

**Caveat:** This rule depends on `normalize syntax query`. As of 2026-05-29, `syntax query` silently ignores top-level `[...]` alternation (see the P0 bug entry in TODO.md, committed 3b8e8857). The workaround is to run each alternation branch as a separate query and merge results.

---

## Appendix: Ecosystem Evidence Index

| Codebase | The signature concretely | Drift? |
|---|---|---|
| **crescent `static/`** | 16-way `solve_*` handler table keyed on constraint-kind (`solve.lua:3993` `get_handlers`); `solve_index` ~665 LOC; `$`-intrinsic if-chain (`intrinsic.lua:912`); CLAUDE.md confesses the `is_numeric_tid` violation (git-confirmed). Canonical worked example. | Not measured — the whole architecture was the finding |
| **wick** | 11 parallel `*_func_name` tables in `wick-scalar`, no shared registry | Yes: `inverse_lerp`/`remap` absent cuda/hip; `cbrt`/`fma`/`hypot`/`copysign`/`asinh` absent CPU backends |
| **marinada** | 4 `switch(arr[0])` passes over `Expr` with no visitor (`evaluate.ts:302`, `typecheck.ts:1027` `_infer`→`inferOp:1701`, `typecheck.ts:3387` `walkLin`, jit, optimizer) | Yes: 46 ops typed-not-evaluated |
| **rescribe** | 3 parallel `match fmt` arms bypassing `Parser`/`Emitter` traits (`rescribe-cli/src/main.rs:805/874/900`) | Not measured |
| **reincarnate** | `EngineOrigin`→string match (`main.rs:509-514`); disasm bypasses `Frontend` pipeline (`main.rs:1876`) | Not measured |
| **paraphase** | `estimate_memory` matches `converter_id` prefix, bypassing `ConverterDecl.costs` (`executor.rs:675-688`) | Not measured |
| **normalize itself** | See TODO.md — violates its own CLAUDE.md in `normalize-facts`, `normalize-deps`, `normalize-refactor`, and `normalize-filter` | Partial: JS/TS deps bypass the `.scm` path while all other languages use it |
