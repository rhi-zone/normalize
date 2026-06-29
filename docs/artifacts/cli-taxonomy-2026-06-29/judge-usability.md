# Judge — Usability & the Grab-Bag/Swell Problem

Lens: human/agent **discoverability** and **navigability**. A taxonomy can be perfectly
objective and still be miserable to use. This judgment ignores migration cost and
"objectivity-of-the-rule" except where they touch usability; it asks one thing of each
tree: *can a user (or an LLM agent reading `--help` text) guess the verb, and once there,
can they scan the verb's children?*

Not committed. Do not commit. No git operations performed.

---

## Attack 1 — The discovery walk (6 real intents → first-verb guess)

A user/agent reaches for a verb *before* they know the answer's data shape. I walked six
real intents through each tree and scored the first guess: ✓ guessable, ~ plausible but
not obvious, ✗ counterintuitive.

| Intent (what the user says) | A (subtract) | B (shape) | C (task) | D (scope) |
|---|---|---|---|---|
| "how complex is this code" | `rank complexity` ~ | `rank complexity` ~ | `rank complexity` ~ | `view complexity` ✗ |
| "find my worst files" | `rank files` ✓ | `rank files` ✓ | `rank files` ✓ | `index files` ✗ |
| "is my PR too big" | `check budget` ✓ | `check budget` ✓ | `check budget` ✓ | `config budget check` ✗✗ |
| "what changed over time" | `rank … --over-history` ✗ | `trend` ✓ | `trend` ✓ | `history trend` ✓ |
| "inspect this function" | `view` ✓ | `view` ✓ | `view` ✓ | `view` ✓ |
| "is the architecture circular" | `view graph` ~ | `graph …` ~ | `analyze architecture` ✓ | `index graph` ✗ |

Rough first-guess hit rate: **C ≈ 5.5/6, B ≈ 4.5/6, A ≈ 4/6, D ≈ 2/6.**

Decisive observations:

- **Nobody thinks in data shapes (kills B's pitch on its own turf).** "How complex is
  this code" is a complexity question, not a "give me a `Vec<Scored<T>>`" question. A/B/C
  all file it under `rank`, which is a ~ for all three — the user has to learn that
  "ranked list" is the home of "complexity." B's deeper problem is the *verb names*:
  `graph cfg`, `tree size` use shape words as verbs. English wants a noun (`cfg`) or a
  task (`analyze`) there. B itself concedes this is its decisive weakness, and the walk
  confirms it — `graph`/`tree` are the only B verbs that aren't guessable.

- **D loses the walk outright.** Scope is a *prerequisite* axis, not an *intent* axis.
  "Is my PR too big" is a pass/fail verdict, but D files `budget` under `config` because
  budgets are `.normalize` state — so the most verdict-shaped intent in the set lands
  under the least verdict-shaped verb. "How complex" → `view complexity` (per-file, so
  S0) while "what's my module surface" → `index surface` (S1) splits one
  intuitively-single "code metrics" family across two verbs on an implementation detail
  the user cannot see. D's own §5 admits all of this. Users do not think "what does this
  command *consume*"; they think "what do I want to *know*."

- **A's trend-as-a-flag is a real miss.** Folding `trend complexity` into
  `rank complexity --over-history` buries a top-level intent ("how is this trending") in
  a flag. A time question should be guessable as a verb. B/C/D all keep it discoverable.

- **C wins discovery because its verbs ARE the questions.** It is also the only tree that
  already matches the shipped guides (`explore`/`analyze`/`rules`/`setup`) — the project's
  own written statement of how users think.

---

## Attack 2 — Grab-bag / swell audit (largest verb per candidate)

| Candidate | Largest verb | ~count | Sub-axis provided? | Second grab-bag? |
|---|---|---|---|---|
| A | `rank` | **~30** | No — concedes "intra-rank categories needed" | **Yes — `admin` (~15+), explicitly unnamed/unmodeled** |
| B | `rank` | **~22** | shape spreads the rest across graph/tree/check/trend | No (resource verbs untouched) |
| C | `rank` | **~22** | Yes — existing topic categories (quality/structure/git/testing) | `manage` (~12-15), but non-navigated |
| D | `index` | **~25** | **No — concedes scope offers no honest sub-axis** | No |

The defining finding here: **only B and C provide a working second level for their
biggest verb, and only B's primary axis subdivides the analysis surface for free.**

- **A has two grab-bags and names neither well.** `rank` at ~30 is not scannable, and A
  punts ~half the services into an `admin` tier it openly refuses to call a verb. An
  unnamed bucket of 15+ heterogeneous commands is *worse* than D's bloated-but-named
  `index` — the user has no entry word at all. A self-admits: "did we subtract concepts
  or just move the depth?" Answer: moved it, and added a homeless tier.

- **B is the best-distributed tree.** Because there are several shapes
  (rank/graph/tree/check/trend), the analysis commands that would otherwise pile into one
  verb spread across five. No single verb exceeds ~22, and the resource/infra verbs
  (`rules`, `config`, `daemon`) are left as their own homes, so there is no admin
  grab-bag. This is B's genuine, under-sold strength — *the axis itself does the
  sub-division work the other candidates defer to a second level.*

- **D's `index` is the most damning grab-bag** precisely because the frame *structurally
  cannot split it*: D's §5 shows that splitting on the S1/S2 prerequisite buys a smaller
  help page with no user-visible meaning (both tiers come from one `structure rebuild`).
  So D ships either a 25-entry junk drawer or smuggles back the very what-it-computes
  distinction it claimed to dissolve, as untested help-text sections. The new `index` is
  the old `analyze`, renamed to a word nobody guesses.

- **C's `manage` grab-bag is the least harmful** because it is the bucket nobody navigates
  by guessing (you don't discover `daemon` by intuition). A grab-bag of things-you-look-up
  is tolerable; a grab-bag of things-you-explore (A's `rank`, D's `index`) is not.

**A 30-deep verb just relocates the navigation problem.** Both A and D concede this and
neither provides the relocation's landing pad. That is the core usability failure of the
single-axis frames.

---

## Attack 3 — One level vs two levels (and what the second level should be)

**The tree is already two-level in practice** and the audit proves it: `rank --help`
ships topic categories (Code quality / Module structure / Repository / Git history /
Testing) and `analyze --help` ships its own (Repository / Graph / Git history / Security /
Diff). The flat single-axis candidates (A's `rank`, D's `index`) are *regressions* — they
collapse an existing, working second level into one unscannable list.

So the live question is not "should there be a second level" — there already is one — but
"what is the right second-level axis?" Candidates:

- **scope-within-task** (`rank` → single-file / index / git): useless. D's own §5 argues
  the prerequisite is an implementation detail, not a user concern. Reject.
- **shape-within-scope** (D + B, `index` → rank/graph/tree/...): this is what D recommends
  as its rescue ("strongest if paired with a secondary axis borrowed from another
  candidate"). It is a *good second level under a bad first level* — the top verb `index`
  failed the discovery walk, so you've put a clean drawer inside an unlabelled cabinet.
- **topic-within-task** (`rank` → quality / structure / git / testing): matches what ships
  today, matches how users explore ("I'm looking at import structure" → the structure
  group), and matches the guides. **This is the right second level.**

**Verdict: two-level, with topic categories as the enforced second level.** The strongest
hybrid is **C's task-named primary verbs + a tested, load-bearing topic second level inside
the populous verbs (`rank` above all), with B's shape rule used only as the mechanical
tiebreak for which verb a command mounts under.** Concretely:

1. Primary verbs = the small intuitive set users actually type (`view`, `rank`, `check`,
   `edit`, `trend`, `grep`, plus the resource/infra homes). These won the discovery walk.
2. Within `rank` (and any verb that swells past ~10), topic categories become a *real,
   snapshot-tested* grouping — not the untested help-text labels that exist today (the
   ones whose staleness produced H-4/H-5). The grouping is documentation that ships and is
   tested, so it can't silently rot.
3. Membership (which verb) is decided by B/C's mechanical shape test ("sorted `Vec` →
   `rank`; verdict → `check`; edges → graph-family"), so the verb assignment can't drift
   on vibes — that is what caused the original mess.

This keeps the human-guessable verb on the outside and the explore-by-topic structure on
the inside, and uses shape only where it belongs: as the drift-proof *rule*, not as the
*name the user has to guess*.

---

## Attack 4 — Eliminating `analyze` (usability call)

Disruption-to-muscle-memory ranking: **C (keeps) < A/B (remove) < D (renames to `index`).**
But muscle memory is a weaker argument than it looks, and the right call is not "least
disruptive":

- **The high-traffic `analyze` muscle memory is *already wrong*.** H-4/H-5: the popular
  subcommands (`analyze complexity`/`length`/`duplicates`) already moved to `rank`, and
  the guides still point at the dead paths. The familiar invocations are broken *today*.
  What remains under `analyze` (health/summary/security/liveness/effects/architecture) is
  the lower-traffic residue.

- **`analyze` is a topic word masquerading as a verb** — it is the exact vague bucket the
  audit blames for the blurry boundary. Several of its residents read *better* under an
  intuitive verb: `check security`, `check dead-code`, `check effects` are more guessable
  than `analyze security` (they are "is something wrong?" questions). So B's elimination
  (route findings → `check`, graph → graph-family) is, for those members, a usability
  *gain*, not a loss. The only awkward residue is the dashboards (`health`/`summary`/`all`),
  which are genuine composites with no clean home in any candidate.

- **D's rename to `index` is the worst usability outcome:** it swaps a vague-but-familiar
  English word for an implementation-detail word (`index`) that no user guesses. It keeps
  the grab-bag and removes the only thing the old name had going for it (familiarity).

- **Given pre-1.0 + retire-don't-deprecate**, disruption is cheap and aliases are banned,
  so "most correct end state" beats "least disruptive." The correct end state retires
  `analyze` *as a verb* (it never named a shape or a question — only a topic) and rehomes
  its members to intuitive verbs, with `check` absorbing the findings-shaped majority.

**Call: eliminate `analyze` as a verb (good for usability), routing findings → `check`,
graph → graph-family, dashboards → wherever composites land. Keeping it (C) is safe but
perpetuates the drift-prone bucket; renaming to `index` (D) is the worst of all worlds.**

---

## Attack 5 — Agents vs humans: do they want different trees?

The intuitive answer is "agents want shape-first (predictable output contract per verb),
humans want intuitive grouping" — which would favor B for agents. **That answer is wrong
for *this* project,** and CLAUDE.md is the reason:

- **"Text output is the agent interface."** LLM agents — the primary programmatic consumer
  here — consume the same `format_text()` as humans and discover the same way: by reading
  `--help`. An LLM does *not* pre-commit to "I want a `Vec<Scored<T>>`"; it thinks "I want
  complexity," exactly like a human. So for the dominant agent class, the discovery walk
  (Attack 1) applies unchanged, and **C wins for agents for the same reason it wins for
  humans.**

- **The shape-predictability argument only serves the *scripted* consumer** (jq pipelines,
  LSP, codegen) — a minority that reads docs once and pins commands. And that need is
  **already met without making shape the verb axis**: server-less exposes `--output-schema`
  on every command (audit, "Root-Global Flag Noise"). A script can introspect any command's
  output contract regardless of which verb it lives under. The schema contract rides on the
  *flag*, not the *verb* — so B's central differentiator is recoverable in any taxonomy and
  is not a reason to distort the verb axis.

**Conclusion: agents and humans want the *same* tree here.** "Text is the agent interface"
collapses the human/agent distinction, and `--output-schema` covers the residual scripted
case. The best taxonomy does not fork by consumer — which removes B's strongest claim to
being the agent-optimal choice.

---

## Per-candidate usability verdict

- **A (subtract — view/rank/check/edit/admin):** Clean four-sentence core, but **two
  grab-bags**: `rank` at ~30 (no provided second level) and an explicitly unnamed `admin`
  tier of ~15+ homeless commands. Trend-as-a-flag buries a real intent. Self-admits it
  "moved the depth." Largest verb: **`rank` ~30** (+ unmodeled admin). Net: the minimal
  core is elegant; the unmodeled half of the tool sinks it.

- **B (shape — verb = output shape):** **Best-distributed tree** (~22 max, analysis spread
  across rank/graph/tree/check/trend, no admin grab-bag) and the most mechanical membership
  rule. But the **verb *names* fight intuition** — `graph`/`tree` as verbs are unguessable,
  and B's headline (predictable schema per verb) is undercut by "text is the agent
  interface" + `--output-schema`. Largest verb: **`rank` ~22**. Net: structurally the most
  navigable, linguistically the least guessable.

- **C (task — view/grep/rank/analyze/trend/edit/check/manage):** **Wins the discovery walk
  (~5.5/6)** and is the only tree that matches the shipped guides. Provides a working
  second level (topic categories already in `rank`). `manage` grab-bag is the harmless
  kind (non-navigated infra). Risk: keeps `analyze`, the vague bucket that caused the
  drift; viable only *with* the mechanical procedure + lint it proposes. Largest analytic
  verb: **`rank` ~22**; grab-bag: **`manage` ~12-15**. Net: most usable, on condition the
  membership rule is enforced.

- **D (scope — view/index/history/fleet/config):** **Most objective, least usable.** Loses
  the discovery walk (~2/6): `config budget` for "is my PR too big," `index architecture`
  for "is it circular," `view complexity` vs `index surface` splitting one metric family.
  `index` is a ~25-command grab-bag the frame **structurally cannot subdivide**. Scope is
  an error-layer concern (T1-1), not a verb axis. Largest verb: **`index` ~25, no
  sub-axis**. Net: solves the drift bug by making the verb un-guessable — wrong trade.

**Usability ranking: C > B > A > D.**

---

## Bottom line

- **One-level vs two-level: two-level, decisively.** The shipped tree is already
  two-level; A and D regress it into unscannable flat verbs. The second level should be
  **topic-within-task** (the categories `rank` already ships), made tested/load-bearing so
  it can't rot like the guides did.
- **The synthesis the usability lens points to:** C's human-guessable verb *names* + B's
  mechanical shape *rule* as the drift-proof tiebreak + an enforced topic second level
  inside the swollen verbs. Use shape as the rule, not the name; use scope in the error
  layer, never the verb; retire `analyze` as a verb and rehome its findings to `check`.
- **Agents and humans want the same tree** — "text is the agent interface" collapses the
  distinction, and `--output-schema` covers the scripted minority, so B's agent-optimality
  claim does not hold.
