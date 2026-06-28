# Design C — Invert the dependency: the macro owns rendering end-to-end

**Date:** 2026-06-28
**Frame:** INVERT THE DEPENDENCY.
**Companions:** `pretty-wiring-audit.md` (the defect census), `diagnosis.md` (the
`sessions stats` instance).

> Premise of this frame: today the *author* owns the rendering plumbing —
> declare `pretty`/`compact` params, call `resolve_pretty`, set `self.pretty`
> (a `Cell`), and point `#[cli(display_with = "fn")]` at a hand-written dispatch
> fn. Four moving parts, every one optional, every omission silent. Flip the
> ownership: the **macro** resolves flags + root and calls the renderer
> uniformly; the author writes a method that returns typed data and *nothing
> else*. You cannot forget plumbing you do not write.

---

## 0. Evidence grounding (what is verified)

- `display_with` is parsed from `#[cli(display_with = "fn")]` per method
  (`server-less-macros/src/cli.rs:483` `get_display_with`). When present, the text
  branch is `let __display = self.#display_fn(&value); println!(...)` (cli.rs:1922–1926).
  When absent, the macro falls back to `Display`/`Vec`/`Map` printing (cli.rs:1927–1947).
- The macro already generates the *entire* leaf arm: arg extraction (cli.rs:1715–1815),
  the method call `self.#method_name(#args)` (cli.rs:1866–1898), output-format-flag
  extraction `--json/--jsonl/--jq` (cli.rs:1902–1906), and the JSON render via
  `cli_format_output` (cli.rs:1949–1958). It **knows the return type tokens**
  (`inner_ty` = ok/some/ty, cli.rs:1910–1917) and generates the call site. It can
  therefore generate the *whole* render step.
- Global flags (`pretty`, `compact`) are registered on the root and propagated into
  `sub_matches` (cli.rs:1003–1115). They are readable unconditionally via
  `sub_matches.get_flag("pretty")` — the macro does this today **only** when the
  method also declares the param (cli.rs:1720–1732). The value reaching the body is
  gated on the per-method param; reading it in the macro is not.
- `ParamInfo` (`server-less-parse/src/lib.rs:60`) carries `name`, `ty`, `is_optional`,
  `is_bool`, `is_positional`, `short_flag`, `help_text`, `wire_name`. `#[param(...)]`
  is an extensible attribute set (`parse_param_attrs`, server-less-parse/src/lib.rs:585;
  the recognised-key list at :689 is where a new key is added).
- `resolve_pretty(root, pretty, compact)` = `!compact && (pretty ||
  NormalizeConfig::load(root).pretty.enabled())` (normalize `service/mod.rs:84`). Every
  method today passes its own `root` param, defaulting to `Path::new(".")`.
- `OutputFormatter` (`normalize-output/src/lib.rs:94`): `format_text()` required,
  `format_pretty()` defaults to `format_text()`.

---

## 1. The design: who owns what after inversion

### The new contract

A service that renders typed reports declares **one** thing, **once**, on the impl
block:

```rust
#[cli(name = "sessions", description = "...", render)]   // <-- the only new token
impl SessionsService { ... }
```

`render` (a bare flag in the `#[cli(...)]` impl attribute) switches every method in
that impl into **renderer mode**. In renderer mode the author writes, per method:

- the parameters the method actually needs for its work,
- a body that returns `Result<R, String>` (or `R`, `Option<R>`) where `R: OutputFormatter`.

The author writes **none** of: `pretty: bool`, `compact: bool`, `self.pretty.set(...)`,
`resolve_pretty(...)`, `#[cli(display_with = "...")]`, a `display_output`/`display_analyze`
fn, or a `pretty: Cell<bool>` field. Those are deleted from the codebase.

### What the macro generates (per method, in renderer mode)

Replacing the `display_with`/Display branch of `gen_value_display` (cli.rs:1920–1947):

```rust
// text branch, renderer mode:
let __flags = ::server_less::CliTextFlags {
    pretty:  sub_matches.get_flag("pretty"),
    compact: sub_matches.get_flag("compact"),
    root:    #root_expr,                        // see §4
};
println!("{}", ::server_less::CliTextRender::render_text(self, &value, &__flags));
```

The JSON/jsonl/jq branch is unchanged (it already wins, cli.rs:1949–1958). `--pretty`
and `--compact` are auto-registered as globals on a `render` impl (the macro adds them
to `global_flags` itself — the author does not even write the `global = [...]` list).

### What the consumer (normalize) writes — once

server-less defines (consumer-agnostic):

```rust
// server-less
pub struct CliTextFlags { pub pretty: bool, pub compact: bool, pub root: Option<std::path::PathBuf> }

/// Generic over the report type T so server-less never names a consumer trait.
pub trait CliTextRender<T> {
    fn render_text(&self, value: &T, flags: &CliTextFlags) -> String;
}
```

normalize provides the policy **once**, as a blanket impl keyed on a marker:

```rust
// normalize, one place (service/mod.rs)
pub trait NormalizeRendered {}                      // marker for rendering services

impl<S: NormalizeRendered, T: OutputFormatter> CliTextRender<T> for S {
    fn render_text(&self, v: &T, f: &CliTextFlags) -> String {
        let root = f.root.as_deref().unwrap_or_else(|| std::path::Path::new("."));
        if resolve_pretty(root, f.pretty, f.compact) { v.format_pretty() } else { v.format_text() }
    }
}
```

…and tags each rendering service with `impl NormalizeRendered for SessionsService {}`
(one line per service, ~9 lines total). `resolve_pretty` — the config + TTY logic —
now lives in exactly **one** call site for the whole binary.

### The ownership flip, stated plainly

| concern | before (author owns) | after (macro/consumer-policy owns) |
|---|---|---|
| read `--pretty`/`--compact` | per-method params | macro, unconditional |
| resolve config + TTY | per-method `resolve_pretty` call | one blanket-impl call site |
| pretty/text dispatch | per-method `display_with` fn | macro-generated, uniform |
| carry the resolved flag | `self.pretty: Cell<bool>` | a stack `CliTextFlags`, no interior mutability |
| author's per-method job | 4 plumbing pieces | **zero** |

---

## 2. Concrete before / after

### BROKEN command: `sessions stats` (the bug that triggered the audit)

**Before** (`service/sessions.rs:242`, abridged): no `pretty`/`compact` params, no
`resolve_pretty`, relies on `#[cli(display_with = "display_output")]` reading a
`self.pretty` that is never set → always `format_text()`.

```rust
#[cli(display_with = "display_output")]
pub fn stats(&self, /* ... 14 work params ... */) -> Result<SessionAnalysisReport, String> {
    // no self.pretty.set, no pretty/compact params  <-- the silent omission
    crate::commands::sessions::build_stats_data(/* ... */)
}
```

**After** (renderer mode): the method is pure data; the omission is impossible
because there is nothing to write.

```rust
pub fn stats(&self, /* ... 14 work params, incl. root ... */) -> Result<SessionAnalysisReport, String> {
    crate::commands::sessions::build_stats_data(/* ... */)
}
```

`--pretty` now reaches `SessionAnalysisReport::format_pretty()` for free, because the
macro generated the resolve+dispatch and the report already implements `format_pretty`.

> Note: `stats` has two early-exit branches (`by_repo`, `group_by`) that today call
> `self.display_output(&report)` then `process::exit(0)`. These render *inside* the
> method and must be refactored to return data — see §6 (this is the genuinely hard
> case the frame surfaces, not hides).

### WORKING command: `sessions list` (correct today, but over-plumbed)

**Before** (`service/sessions.rs:89`): declares `pretty`/`compact`, calls
`resolve_pretty`, routed via `display_output`.

```rust
#[cli(display_with = "display_output")]
pub fn list(&self, /* work params */, pretty: bool, compact: bool) -> Result<SessionListReport, String> {
    let resolved_root = root.as_deref().map(Path::new).unwrap_or(Path::new("."));
    self.pretty.set(resolve_pretty(resolved_root, pretty, compact));   // <-- boilerplate
    /* ... build report ... */
}
```

**After**: drop two params, one `set`, and the `display_with` attribute. The method
shrinks to its actual job.

```rust
pub fn list(&self, /* work params, incl. root */) -> Result<SessionListReport, String> {
    /* ... build report ... */
}
```

Identical behaviour, less surface, nothing left to forget.

---

## 3. How each defect class becomes impossible (or dissolves)

### (a) SILENT NO-OP — *impossible by construction*

The defect was: `format_pretty()` never dispatched because the author forgot the
params + `self.pretty.set`. After inversion the author writes no flag plumbing at all;
the macro unconditionally extracts the flags and calls `render_text`, which is the only
text path. There is no opt-in to omit, so there is nothing to forget. The `Cell` that
defaulted to `false` is deleted, so "construction-time copy stuck at false" (the
`grammars` latent bug, audit Part 1b §3) cannot exist either.

### (c) DEAD DISPATCH — *impossible by construction*

The defect was a hand-written `display_with` fn that calls `format_text()`
unconditionally (the 6 `edit` refactor commands + `syntax node-types`, audit Part 1b
§1–2). After inversion `display_with` is **removed**; there is no author-written
dispatch fn to get wrong. The single generated render path is conditional by
construction (`if resolve_pretty(...) { format_pretty } else { format_text }`). Those 7
unreachable `format_pretty` implementations light up the moment their methods switch to
renderer mode — at zero per-method cost.

### (b) ADVERTISED NO-OP — *downgraded from defect to non-defect* (honest)

The defect was: `--pretty` advertised but no report implements a distinct
`format_pretty`, so the flag does nothing. This one **cannot be made a compile error** —
the macro cannot tell whether a type's `format_pretty` differs from its `format_text`
(both compile; the bodies are opaque to the macro). The inversion instead removes the
*defect character* of (b): `--pretty` is no longer something a service "advertises and
fails to honour." In renderer mode every command's `--pretty` is genuinely wired to
`format_pretty()`. If a report did not override `format_pretty`, `--pretty` renders the
documented `OutputFormatter` default (== text) — an honest identity, not a broken
promise. So (b) stops being "advertised but disconnected wiring" and becomes "the type
chose not to differentiate," which is a content decision, not a wiring bug. This is the
one place the frame is thin (see §6).

> A weak static aid is still possible if desired: a derive/test that flags report
> types which `impl OutputFormatter` *without* overriding `format_pretty` yet are
> returned by a `render` method — surfacing (b) as a lint, not a hard error. Optional;
> not required by the inversion.

---

## 4. Root-aware + TTY resolution — the crux

`resolve_pretty` needs the path config is rooted at. Once the method no longer takes
`pretty`/`compact`, the macro must still hand the renderer a **root**. Resolution order
the macro applies, per method, decided at macro-expansion time from `ParamInfo`:

1. **Explicit annotation (authoritative).** A param marked `#[param(config_root)]`
   (a new key added to `parse_param_attrs`, server-less-parse/src/lib.rs:689) names the
   config root. Used for commands whose config root is neither cwd nor a param literally
   named `root` (e.g. `view <path>`, `analyze <path>`, where the *positional target* is
   the root).
2. **Name convention (reuse what's already there).** Else, if a param is literally
   named `root`, use it. This is the dominant case — nearly every audited method already
   has `root: Option<String>` (sessions, analyze, rank …). The macro reuses the work
   param the author already wrote; no new annotation needed for ~all current commands.
3. **cwd fallback.** Else `None` → the renderer defaults to `Path::new(".")`. Correct
   for commands that genuinely operate on cwd and take no path.

**Type coercion (the bounded-hard part).** Root params vary in type (`Option<String>`,
`String`, `Option<PathBuf>`, …). Rather than branch codegen per type, server-less ships
a conversion trait with blanket impls for the common types:

```rust
// server-less
pub trait AsConfigRoot { fn as_config_root(&self) -> Option<std::path::PathBuf>; }
impl AsConfigRoot for Option<String> { /* map PathBuf::from */ }
impl AsConfigRoot for String         { /* Some(PathBuf::from) */ }
impl AsConfigRoot for Option<std::path::PathBuf> { /* clone */ }
impl AsConfigRoot for std::path::PathBuf { /* Some(clone) */ }
impl AsConfigRoot for &str { /* Some(PathBuf::from) */ }
```

The macro then emits a single uniform expression regardless of the param's type:

```rust
// #root_expr, when a root param `root` was selected:
::server_less::AsConfigRoot::as_config_root(&root)
// when none selected:
None
```

Trait dispatch picks the right conversion; the macro stays type-agnostic. The flag value
extraction is already in scope (the param was extracted at cli.rs:1715–1815 before the
call), so `&root` is a live binding in the arm.

**TTY** is unchanged: `NormalizeConfig::pretty.enabled()` inside `resolve_pretty`
already consults TTY/config. Because the renderer calls `resolve_pretty` with the
resolved root, TTY auto-enable now works for *every* renderer-mode command — including
`stats`, which the audit notes was bypassing TTY detection entirely.

---

## 5. Migration plan, cost, risk, blast radius

### server-less (additive — not breaking)

Add `CliTextFlags`, `CliTextRender<T>`, `AsConfigRoot` (+ blanket impls), the
`render` impl-attribute flag, the `#[param(config_root)]` key, and the renderer-mode
branch in `gen_value_display`. **Keep `display_with` and the Display fallback exactly as
they are.** `render` is opt-in per impl; a consumer that does not set it sees no change.

> **Blast radius for other server-less consumers: zero.** This is the decisive reason to
> add a mode rather than replace `display_with`. Removing `display_with` would break every
> external consumer that uses it; the audit (Part 2, "Alternative considered") flagged the
> signature-change route as "a larger migration." Additive renderer mode avoids that
> entirely. Internal-only follow-up (optional): once normalize is fully migrated, the
> `display_with` machinery has no in-tree consumer and *may* be retired per the
> retire-don't-deprecate tenet — but that is a separate, later decision and does not gate
> this work.

### normalize

| step | scope | nature |
|---|---|---|
| add `NormalizeRendered` marker + one blanket `CliTextRender` impl + `resolve_pretty` reuse | 1 place | additive |
| tag rendering services `impl NormalizeRendered for X {}` | ~9 services | 1 line each |
| add `render` to each rendering impl's `#[cli(...)]` | ~9 impls | 1 token each |
| delete `pretty`/`compact` params + `resolve_pretty`/`self.pretty.set` calls | ~46 working + ~16 broken methods | net deletion |
| delete `#[cli(display_with = "...")]` on report methods | ~62 methods | deletion |
| delete `display_output`/`display_analyze`/per-service display fns | ~each service | deletion |
| delete `pretty: Cell<bool>` field + `new(&pretty)` wiring | every service + `NormalizeService` | deletion |
| add `#[param(config_root)]` where the root is a positional (not named `root`) | handful (`view`, `analyze <path>`, …) | 1 attr each |

The ~16 broken commands need **no special fix** — deleting their plumbing and switching
to renderer mode makes them correct, which is the whole point of the frame. The ~46
working commands are pure simplification with identical behaviour.

### Risks

- **In-method render + `process::exit`.** `stats` (by_repo / group_by) and any sibling
  that prints inside the body then exits bypass the macro renderer. These must be
  refactored to *return* a report (fold `RepoStatsReport` and grouped output into the
  return type, likely an enum report implementing `OutputFormatter`). Medium effort,
  isolated to ~2–3 methods. This is real and the frame does not erase it; it pushes
  these toward the API-first ideal (return data, let the CLI render).
- **Root convention mis-selection.** A method with a param named `root` that is *not*
  the config root would resolve config against the wrong path. Mitigation: explicit
  `#[param(config_root)]` always wins; audit the name-convention hits once during
  migration.
- **`AsConfigRoot` type coverage.** Must enumerate every root-param type in the tree;
  an unmatched type is a compile error (fail-loud, not silent) — acceptable.
- **Mixed-return impls.** A `render` impl whose methods return non-`OutputFormatter`
  types: the `CliTextRender<T>` bound fails to resolve → compile error pointing at that
  return type. Unit returns are already special-cased by the macro (`"Done"`,
  cli.rs:1964) and skip the render path. So `render` impls must contain only
  report-returning (or unit) methods; split a service if it mixes.

---

## 6. Honest trade-offs

**Where inversion is strong.**
- (a) and (c) become *structurally impossible*, not "caught by a test." The defect that
  recurred eight times (stats, subagents, architecture, cross_repo_health, 4× rank)
  cannot recur, because the plumbing it omitted no longer exists.
- One source of truth for pretty resolution (`resolve_pretty` called in exactly one
  blanket impl) instead of ~62 copy-pasted call sites.
- The method becomes pure data return — the literal realisation of "normalize is an API
  that happens to have a CLI." The `Cell<bool>` interior mutability disappears; flags
  flow in via a stack struct, not out via a mutated field.
- Net deletion of code; the 7 already-written-but-unreachable `format_pretty`
  implementations light up for free.

**Where it is thin.**
- **(b) cannot be a compile error.** No macro can prove `format_pretty != format_text`.
  Inversion dissolves (b) into a non-defect (the flag is always honestly wired) rather
  than catching it. If a hard guarantee is wanted, only a lint (§3b) is available.
- **Custom dispatch must become data.** Commands that print mid-body and `exit`
  (`stats` by_repo/group_by) don't fit "return one report, macro renders." The frame
  forces them to return data — correct in principle, but it is migration work, not a
  free win. Multi-report commands (`plans`, the `stats` variants) need a unified report
  type, which the repo's CLAUDE.md already mandates ("real consolidation means one report
  struct," not an enum-of-unrelated-reports — so fold shared fields, don't just wrap).
- **Error rendering is untouched.** Errors are `Result::Err(String)` rendered by the
  macro as plain stderr text + `exit(1)` (cli.rs:1973–1976). Inversion neither helps nor
  harms here; pretty error rendering remains out of scope (and arguably should stay so).
- **`render`-impl homogeneity.** The mode is per-impl, so a service mixing
  report-returning and raw-`Display`-returning methods must be split or left on the
  legacy path. In practice the audited rendering services are already homogeneous.

**Net.** The frame trades a small, honest gap (b is a lint at best; a few exit-printing
methods need real refactors) for the elimination of the two defect classes that
actually bit users repeatedly — and it does so by *removing* code and interior
mutability rather than adding a new opt-in hook the author could still forget. Compared
to the audit's `CliGlobals` trait-hook sketch (which keeps `display_with` and the
`Cell`, and still relies on each service implementing the hook), this goes one level
deeper: it deletes the forgettable surface instead of making it easier to remember.
