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
