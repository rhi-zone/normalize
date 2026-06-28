# docs/artifacts/sessions-stats-output-2026-06-20

Investigation into `normalize sessions stats --pretty` silently falling back to text,
2026-06-20 / 2026-06-28.

## Contents

- `diagnosis.md` — Root-cause diagnosis of the single `sessions stats` instance: the
  `#[cli]` method omits `pretty`/`compact` params and never calls `self.pretty.set(...)`,
  so `display_output` always picks `format_text()`.
- `pretty-wiring-audit.md` — Workspace-wide audit of the same defect class across every
  `#[cli]` command. Lists 8 BROKEN commands (real `format_pretty` + dispatching display
  fn + unwired flag), the WORKING set, adjacent "unreachable pretty" defects, and a
  structural root-cause assessment proposing a `CliGlobals` auto-wiring hook in
  server-less (verified against the proc-macro source).
- `design-A-subtract.md` — Design (MINIMIZE/SUBTRACT frame): collapse
  `format_text`/`format_pretty`/`display_with`/`self.pretty` Cell/in-body `resolve_pretty`
  into one macro-driven primitive — a single `CliRender::render(&self, RenderMode)` the
  macro always calls, with mode resolved by the macro from the flags (+ TTY + config via a
  one-per-service hook, root threaded via a `#[param(render_root)]` marker). Makes the (a)
  and (c) defect classes impossible by construction and dissolves (b).
- `design-B-type-property.md` — Design (TYPE-PROPERTY frame): pretty-ness becomes one
  type-level fact, `impl PrettyFormat for Report` (a non-defaulted sub-trait split out of
  `OutputFormatter`). The macro emits identical generic code for every command; the
  *compiler* resolves it per return type via inherent-vs-trait specialization — dispatch
  (`Render(&v).render(want)`) and advertising (`<PrettyProbe<RetTy>>::HAS_PRETTY` gates the
  `--pretty` flag) both fall out of that one fact. `global=[pretty]` and all per-method
  Cell/param/`resolve_pretty` plumbing are retired; (a)/(b)/(c) become unrepresentable.
  Includes the honest token-only-macro limit and a verified `rustc` probe of both
  specialization patterns. Root reaches resolution via the macro reading the `root` param
  token into a `PrettyPolicy` hook.
- `design-C-invert.md` — Design (INVERT THE DEPENDENCY frame): the macro owns rendering
  end-to-end. A per-impl `#[cli(..., render)]` flag switches every method into renderer
  mode; the method returns pure typed data and writes zero flag/Cell/display plumbing.
  The macro extracts `--pretty`/`--compact`, resolves the config root (via
  `#[param(config_root)]` > a param named `root` > cwd) coerced through an `AsConfigRoot`
  trait, and calls a consumer-supplied `CliTextRender<T>` policy (one blanket impl in
  normalize). Removes `display_with` and `self.pretty` Cell. Additive macro change (zero
  blast radius for other server-less consumers); (a)/(c) impossible by construction, (b)
  dissolves into an always-honest flag.
- `design-D-build-guard.md` — Design (BUILD-TIME GUARD / EXHAUSTIVENESS frame): keep the
  runtime dispatch as-is and bolt on a guard layer. Layer 1 is a macro `compile_error!`
  (sibling of `check_reserved_flag_collisions`) enforcing "impl declares
  `global=[pretty,compact]` ⇒ every method declares the params" — makes defect (a) a hard
  compile error, guarded itself by trybuild cases. Layer 2 replaces the hand-maintained
  `assert_output_formatter` list with a macro-emitted `inventory` manifest + exhaustiveness
  test (catches (a), and (b) via a `HAS_REAL_PRETTY` marker const). Layer 3 is a fixture-
  driven `--pretty`-vs-text snapshot test for (c). Honest verdict: only (a) is genuinely
  compile-time; (b)/(c) are CI tests with real reliability limits; resolution-correctness
  (root/TTY) is detectable but not fixable by guards — pair with `CliGlobals` to fix it.
  Lowest blast radius, but strictly weaker than the trait redesigns on (b)/(c).
- `judge-completeness.md` — Adversarial bake-off judgment: for each design, the concrete
  defects that SURVIVE it (with escape code) and a survivor count. Identifies two universal
  residuals no design closes via types — U1 (pretty bytes == text bytes) and U2 (body
  bypasses the render path) — both needing a behavioural CI test. Ranks B (eliminates
  false-advertising structurally) ≳ A > D (detects, doesn't prevent) > C (retains the
  defaulted `format_pretty` root cause). Recommends grafting D's `compile_error!(a)` as an
  interim bridge (it becomes dead once A/B/C delete the `global`/param tokens it inspects)
  and adopting D's behavioural distinctness test permanently regardless of winner.
