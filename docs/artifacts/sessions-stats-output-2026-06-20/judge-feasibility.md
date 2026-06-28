# Adversarial feasibility judgment — four pretty-wiring designs

**Date:** 2026-06-28
**Role:** adversarial technical judge. Mandate: attack the *type-system and proc-macro
mechanisms* each design relies on, with real `rustc` probes — not reasoning from memory.
**Probes:** `rustc 1.95.0`, `--edition 2021`. Probe sources kept under the session
scratchpad; the load-bearing snippets and exact compiler output are pasted inline below.

---

## TL;DR verdicts

| design | core mechanism | verdict | the one thing that decided it |
|---|---|---|---|
| **A — subtract** | macro owns one `render(mode)` call; root threaded from a `render_root` param | **SOUND-WITH-CAVEAT** | the `render_root` param is **moved into the method call** before the render site → `E0382` as written (probe `move.rs`). Fixable. |
| **B — type-property** | dual inherent-beats-trait specialization (dispatch + advertise) | **SOUND-WITH-CAVEAT** | **both halves compile and resolve correctly at concrete types** (probes `b_dispatch.rs`, `b_advertise.rs`). But they degrade silently to the fallback in *generic* context (probe `b_generic.rs`), and B's blast-radius claim is **FALSE** (§B.3). |
| **C — invert** | macro owns render; `AsConfigRoot` blanket impls; marker-trait blanket `CliTextRender<T>` | **SOUND-WITH-CAVEAT** | identical move bug to A: `AsConfigRoot::as_config_root(&root)` borrows a value already moved into the method → `E0382`. Fixable. |
| **D — build-guard** | `compile_error!` in the macro over `global ⇒ params` | **SOUND** | macro provably sees impl-level globals **and** every method's params at the same expansion (`expand` → `partitioned` + `global_flags`); direct precedent `check_reserved_flag_collisions`. Lowest mechanism risk on the table. |

**Least technically risky core mechanism: D.** It is a purely additive `syn::Error`
emitted from a code path (`expand`) that already does exactly this shape. No trait-resolution
trick, no ownership threading across the call boundary, no cross-repo breaking trait split.

**Single most important compiled result:** B's make-or-break dual specialization **works** —
at a concrete return type the inherent method/const beats the trait fallback (Rich→`PRETTY`/
`HAS_PRETTY=true`, Plain→`text`/`false`). The macro *does* emit at a concrete type (each leaf
arm binds `value: <ConcreteReportTy>` from `Ok(value)`), so B's crux holds. The accompanying
negative result matters just as much: in a **generic** `T: OutputFormatter` context the same
call silently resolves to the text fallback — so the pattern is correct *only* because the
emission site is monomorphic, and any future refactor that makes it generic would reintroduce
the silent no-op the design exists to kill.

---

## A — SUBTRACT

**Mechanism claims:** (1) replace `display_with`/`Cell`/`resolve_pretty` with one macro-driven
`CliRender::render(&value, mode)`; (2) an impl-level `render_mode = "fn"` hook (mirroring how
`global`/`display_with` are parsed today); (3) thread the resolved `root` into the resolver via
a `#[param(render_root)]` marker, citing "the macro has already extracted every method
parameter — including `root` — into local bindings before it renders (cli.rs arg_extractions
run before #output)."

### What is true
- The render-call rewrite is feasible. `gen_value_display` (cli.rs:1920-1960) already owns the
  text branch and emits `println!("{}", #value_ident)` at a **concrete** value type; swapping
  that for `CliRender::render(&value, mode)` is a local edit. Confirmed against source.
- The impl-level hook is feasible: `render_mode = "fn"` parses exactly like the existing
  `display_with` (cli.rs:483) and impl-level `global` (cli.rs:607). No new macro capability.
- Arg extractions *do* run before the output block, so the binding `root` *exists by name* at
  the render site. The design's literal phrasing ("in scope") is true.

### The break (verified)
"In scope by name" ≠ "usable." The macro passes regular params **by value** into the method
call: `arg_names` pushes the bare `#name` (cli.rs:1814) and the call is
`self.#method_name(#(#arg_names),*)` (cli.rs:1893). `stats` takes `root: Option<String>` **by
value** (sessions.rs:260). `Option<String>` is not `Copy`, so `root` is *moved* into the call.
The render site runs *after* the call, so reusing `root` there is a use-after-move.

Probe `move.rs` (models exactly this emission):
```rust
fn stats(root: Option<String>) -> Result<String, String> { Ok(format!("{:?}", root)) }
fn main() {
    let root: Option<String> = Some("x".into());   // macro extracts param
    let result = stats(root);                       // macro: self.stats(root, ...) -- MOVE
    match result {
        Ok(value) => {
            let _root_ref = root.as_deref();        // A/C render site reuses root
            println!("{} {:?}", value, _root_ref);
        }
        Err(_) => {}
    }
}
```
Compiler:
```
error[E0382]: borrow of moved value: `root`
 6 |     let result = stats(root);
   |                        ---- value moved here
10 |             let _root_ref = root.as_deref();
   |                             ^^^^ value borrowed here after move
```
**Verdict on the claim "the resolved root is in scope at the render call site":** the *name* is
in scope; the *value* is gone. As written, A's §4 root threading does not compile.

**Severity: caveat, not fatal.** Fix is mechanical and the design already has the template for
it: the macro re-reads machine-format flags from `sub_matches` at the render site
(cli.rs:1902-1906) rather than from moved params. A `render_root` resolver should likewise pull
the raw value from `sub_matches.get_one::<String>("root")` (or the macro must `clone()` the
param before the call). Either is a few tokens. The design author must not "reuse the binding."

### process::exit wrinkle (sized)
`process::exit` inside the service layer is **only** in `sessions stats` (sessions.rs:296,
group_by 322) — 2 of 14 workspace-wide `process::exit` sites; the rest are in dispatch/main.
A's "the only place the subtraction touches data shape" and "~2-3 methods" is **accurate, even
generous** — it is one method, two branches. Note `println!` appears in `service/edit.rs` (15)
and `service/rename.rs` (6); those are display/dry-run side channels, not exit-mid-method, and
A handles them via `display_with` deletion — not a blocker, worth a migration eye.

**A verdict: SOUND-WITH-CAVEAT.** Render rewrite + impl hook are sound and use existing macro
shapes. The `render_root` threading is broken *as specified* (E0382) but trivially fixable by
re-reading from `sub_matches`. process::exit refactor is genuinely bounded to one method.

---

## B — TYPE-PROPERTY (the crux design)

**Mechanism claims:** (i) dispatch via `Render(&value).render(want)` where an inherent method
present only for `T: PrettyFormat` beats a trait fallback; (ii) advertise via
`<PrettyProbe<RetTy>>::HAS_PRETTY` where an inherent assoc const beats a trait-const fallback.
B explicitly stakes everything on a standalone `rustc` probe (§7). I reproduced it and then
attacked it.

### B.1 — Both halves COMPILE and resolve correctly (the make-or-break)

Dispatch probe `b_dispatch.rs` (trait fallback `use`'d into scope, as macro-emitted code would
have it; `value` concrete per arm):
```
Rich  want=true  -> PRETTY
Rich  want=false -> text
Plain want=true  -> plain      <- non-PrettyFormat type: inherent N/A, trait fallback → text
Plain want=false -> plain
```
Advertise probe `b_advertise.rs`:
```
Rich  HAS_PRETTY = true
Plain HAS_PRETTY = false
Rich  advertises --pretty
Plain does NOT advertise --pretty
```
Both compiled cleanly (only `private_bounds`/`dead_code` warnings from my deliberately-private
test traits — vanish with `pub`). **Inherent method beats trait method, and inherent assoc
const beats trait assoc const, at a concrete type, with the fallback trait in scope.** B's load-
bearing claim is real on `rustc 1.95.0`.

**This is sound for the macro because the emission site is monomorphic.** `gen_value_display`
binds `value` from `Ok(value)`/`Some(value)` of `match result`, where `result =
self.#method(...)` returns the concrete report type (cli.rs:1962-1999). For advertising, the
return-type token is available at the builder site too: `generate_leaf_subcommand(m, …)`
receives the `MethodInfo`, whose `return_info.ok_type` is the same token `inner_ty` is derived
from (cli.rs:1908-1917). So `<PrettyProbe<#ok_type>>::HAS_PRETTY` can be emitted where clap args
are built. Both sites have what they need.

### B.2 — The adversarial break: generic context silently picks the fallback

The judge's specific question — does inherent-vs-trait priority hold "when the calling code is
generic over the type"? Probe `b_generic.rs`:
```rust
fn generic_render<T: OutputFormatter>(v: &T, want: bool) -> String {
    use RenderFallback as _;
    Render(v).render(want)          // T only known as OutputFormatter here
}
fn main() { println!("{}", generic_render(&Rich, true)); }  // Rich IS PrettyFormat
```
Output:
```
generic Rich want=true -> text
```
**`Rich` is `PrettyFormat`, yet routed through a `T: OutputFormatter` generic it renders
`text`, not `PRETTY`.** The inherent method requires `T: PrettyFormat`, which the generic
context cannot prove, so resolution falls to the trait method — silently. This is the exact
silent-no-op class the design eliminates, reintroduced if anyone ever makes the emission
generic. **It does not affect B today** (the macro emits concrete tokens), but it is a real,
load-bearing constraint: B's correctness is contingent on the call site staying monomorphic.
This belongs in the server-less doc-comment + a regression test, as B §6 already half-concedes.

### B.3 — FALSE claim: the breaking-change blast radius

B §5 / §6 assert the feature crates "have **no real pretty** … they implement only
`format_text`, so they need **no source change**," naming `normalize-ratchet, normalize-budget,
normalize-rules, normalize-facts, normalize-knowledge-graph, normalize-native-rules,
normalize-syntax-rules`, and conclude "blast radius outside the main crate is therefore
near-zero." Grep of the actual tree contradicts this:

```
normalize-rules/src/runner.rs:334    fn format_pretty   (RulesListReport)
normalize-rules/src/service.rs:227   fn format_pretty   (RulesTestReport)
normalize-rules/src/service.rs:325   fn format_pretty   (RulesFixtureTestReport)
normalize-native-rules/src/budget.rs:23   fn format_pretty  -> self.0.format_pretty()
normalize-native-rules/src/ratchet.rs:23  fn format_pretty  -> self.0.format_pretty()
normalize-context/src/lib.rs:80      fn format_pretty   (ContextReport)
normalize-graph/src/lib.rs:196,1238  fn format_pretty   (DependentsReport, GraphReport)
```

These are genuine `impl OutputFormatter { fn format_pretty … }` overrides. Under B's design —
removing `format_pretty` from `OutputFormatter` and lifting it into a non-defaulted
`PrettyFormat` sub-trait — **every one of these stops compiling** (override of a method that no
longer exists; and the native-rules wrappers' `self.0.format_pretty()` only compiles if the
inner type is `PrettyFormat`). So at least **four additional crates** (`normalize-rules`,
`normalize-native-rules`, `normalize-context`, `normalize-graph`) require source migration —
contradicting "near-zero outside the main crate." B's *cost estimate* is wrong.

**Severity: caveat, not fatal.** The trait split itself is mechanically sound and self-checking
(the compiler enumerates every broken site — that is B's own "migration safety net"). The defect
is in B's accounting, not its mechanism: the migration is wider and the SemVer/coordination cost
higher than B states. (The audit's "ratchet/budget have no real pretty" referred to crates
`normalize-ratchet`/`normalize-budget`; B over-generalized it to `normalize-native-rules`, which
*does* override `format_pretty`.)

**B verdict: SOUND-WITH-CAVEAT.** The dual specialization — the entire risk of the design — is
real and compiles. Two caveats: (1) correctness is contingent on the emission staying concrete
(generic context degrades silently — verified); (2) the "near-zero blast radius" claim is false,
≥4 extra crates migrate. Neither breaks the mechanism; both should be recorded.

---

## C — INVERT

**Mechanism claims:** macro owns render end-to-end via a `render` impl-flag; `CliTextRender<T>`
provided once by a blanket impl keyed on a `NormalizeRendered` marker; root coerced uniformly
via an `AsConfigRoot` trait with blanket impls over `Option<String>`/`String`/`PathBuf`/`&str`,
emitted as `AsConfigRoot::as_config_root(&root)`.

### What is true
- The `render` impl-flag and the renderer-mode branch in `gen_value_display` are the same
  feasible local edit as A. Sound.
- `AsConfigRoot` with blanket impls per concrete type is ordinary stable Rust; trait dispatch at
  the concrete param type resolves the right conversion. No specialization trick needed. Sound.
- The marker-trait blanket `impl<S: NormalizeRendered, T: OutputFormatter> CliTextRender<T> for
  S` is a standard coherent blanket impl (normalize owns all three of the trait, the marker, and
  the impl). Sound. Mixed-return impls fail to resolve the bound → compile error, as C claims.

### The break (verified — same root cause as A)
C §4 emits, at the render site, `::server_less::AsConfigRoot::as_config_root(&root)` — a
**borrow** of `root`. But `root` was moved into `self.stats(root, …)` at the call (cli.rs:1893;
`root: Option<String>` by value, sessions.rs:260). The render site is after the call. Probe
`move.rs` above is exactly this: `root.as_deref()` after `stats(root)` → `E0382: borrow of moved
value`. So `&root` at `#root_expr` does not compile as specified.

**Severity: caveat, not fatal** — identical fix to A: read the raw root from `sub_matches` at the
render site (where the macro already reads json flags), or clone the param before the call. C's
`AsConfigRoot` then converts the *re-read* value, and the type-agnostic-codegen benefit is
preserved. C's claim "`&root` is a live binding in the arm" (§4) is the precise error: it is
*declared* in the arm but *moved* before render.

process::exit: same bounded picture as A (one method). C's "~2-3 methods … isolated" is accurate.

**C verdict: SOUND-WITH-CAVEAT.** Blanket impls (`CliTextRender<T>`, `AsConfigRoot`) and the
marker-trait dispatch are sound stable Rust with no specialization fragility — mechanically the
*calmest* of the render-owning designs. The single defect is reusing the moved `root` binding
(E0382), fixable by re-reading from `sub_matches`.

---

## D — BUILD-GUARD

**Mechanism claim:** a macro `compile_error!` (`syn::Error`) enforcing "impl declares
`global=[pretty,compact]` ⇒ each leaf method declares `pretty: bool, compact: bool` params,"
fires on `stats`, passes on `list`, with `#[cli(no_globals)]` as the sole opt-out.

### Visibility — verified the macro sees both facts at once
`expand` computes `global_flags` (cli.rs:607-611) and then `partitioned = partition_methods(…)`
(cli.rs:631), and immediately calls `check_reserved_flag_collisions(&partitioned, meta)`
(cli.rs:636). `partitioned.leaf` is the full leaf-method list, each `MethodInfo` carrying its
`params` (with `is_bool`, `name`). So **at one expansion the macro has the impl-level globals and
every method's params simultaneously** — exactly what D's `check_global_pretty_wiring(partitioned,
global_flags)` needs. The macro expands per impl block and sees all its methods (confirmed:
`leaf_subcommands` iterates `partitioned.leaf`, cli.rs:656). D's premise is correct.

### Non-bypassability — verified the precedent
`check_reserved_flag_collisions` (cli.rs:384) is a working sibling: it builds a `syn::Error`,
combines multiple, and returns `syn::Result` that `expand` propagates as a `compile_error!`
spanned to the offending param/method. D's guard is the same construction. Because it lives in
the macro that *generates the entire CLI*, there is no command without the guard, no `#[allow]`,
no deletable test. **Non-bypassable as claimed.** Logic check: gated on `governs` (both `pretty`
and `compact` present in globals) — correct; `stats` (globals declared, params absent) fires,
`list` (params present) passes. Sound.

### One inaccuracy (minor)
D's snippet calls `has_cli_no_globals(m)` as if it exists. It does **not** — there is no
`no_globals` handling in cli.rs today. But the codebase has four established method-attr probes
(`has_cli_skip` :308, `has_cli_default` :333, `has_cli_manual_false` :357, `has_cli_hidden`
:458); adding `has_cli_no_globals` is a copy of that pattern. So the opt-out is *not yet built*
but is trivially buildable and consistent with existing macro capability. D presents it as part
of the proposed change, so this is honest, not a false claim — just flagging it doesn't exist
yet.

**D verdict: SOUND.** Every macro capability D relies on is either already present
(cross-method/`global` visibility, `syn::Error` spanning, the `check_reserved` precedent) or a
direct clone of an existing helper (`no_globals`). No type-resolution trick, no ownership
threading, no breaking trait change. D is honest that (b)/(c) drop to CI tests and resolution-
correctness is out of reach — those are scope limits, not feasibility failures. The *mechanism*
is the most robust on the table.

---

## Cross-design summary

- **The one mechanism that could have been vapor — B's dual inherent-beats-trait
  specialization — is real and compiles** at concrete types (dispatch *and* advertise). That was
  the highest-risk claim across all four designs and it survives the probe. Its only fragility is
  the monomorphism constraint (generic context degrades to the fallback — verified), which the
  macro satisfies today.
- **A and C share one concrete compile error**: both reuse the `root` param at the render site,
  but the macro moves owned params into the method call (`E0382`, verified). Both fixable by re-
  reading from `sub_matches` — the exact channel the macro already uses for json flags. Neither
  design author can "reuse the binding" as written.
- **B's only *false* claim** is bookkeeping: ≥4 feature crates (`normalize-rules`,
  `normalize-native-rules`, `normalize-context`, `normalize-graph`) override `format_pretty` and
  *do* need migration under the trait split — "near-zero outside the main crate" is wrong.
- **D is the lowest-risk core mechanism**: additive `syn::Error` from a code path that already
  does this, no trait magic, no cross-repo break. Its honest limit is that it only *detects* (a)
  and pushes (b)/(c) to CI — a scope choice, not a feasibility gap.

**Mechanism-risk ranking (lowest → highest):** D < C < A < B. (C edges A only because
`AsConfigRoot`/marker-blanket are plain stable trait dispatch with no fragility beyond the shared
move bug; B is highest purely because it leans on resolution-priority subtleties — which
nonetheless held.)
