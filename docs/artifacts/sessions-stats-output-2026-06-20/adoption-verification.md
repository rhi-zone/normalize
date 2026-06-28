# Independent verification — server-less 0.6 CliGlobals adoption + --pretty fix

Verifier: independent agent (did not trust implementer self-report).
Branch: `feat/cli-globals-pretty-wiring` @ `9b31c6de`.
server-less: resolved from `[patch.crates-io]` → `/home/me/git/rhizone/server-less/crates/server-less` at **v0.6.0** (confirmed via `cargo tree -p server-less` and `Cargo.lock`).
Date: 2026-06-29.

## Bottom line

**SOUND and ready.** The 8 formerly-broken commands are genuinely fixed. None of the
previously-working commands regressed. Root-aware resolution is correct (verified
empirically). No re-introduced silent no-op. One inaccuracy in the commit message
(item 6) and one pre-existing latent gap (7 analyze methods never resolve pretty) are
noted as CONCERNs — neither is introduced by this migration and neither blocks merge.

## 1. Build / lint / test — PASS

- `cargo build`: clean (only pre-existing dead-code warnings inside `server-less-macros`, not normalize).
- `cargo clippy --all-targets --all-features -- -D warnings`: **exit 0**.
- `cargo test -q`: **all pass** (89 + 6 + smaller suites + doctests; 1 ignored = the known daemon flake, ignored not failed). No failures.
- Build resolves server-less 0.6.0 from the local patch. Since 0.6 makes a method param matching a declared global a **compile error**, the passing build proves every `global`-declaring service has a valid `CliGlobals` impl and no method still takes `pretty`/`compact`.

## 2. The 8 formerly-broken commands — PASS

Method: ran each `--pretty` (piped) vs default (piped) and checked for structural divergence
(ANSI `\x1b`, box-drawing, length/format change).

| Command | Result | Evidence |
|---|---|---|
| sessions stats | FIXED | pretty: 159 ANSI + 10 box-drawing; text: 0 / 0. DIFFER. |
| analyze architecture | FIXED | text = compact `HUBS:/LAYERS:` form; pretty = expanded multi-line report. DIFFER (6273 vs 3616 chars). |
| rank files | FIXED | pretty 2 ANSI vs 0. DIFFER. |
| rank size | FIXED | pretty wraps title in bold `\x1b[1m...\x1b[0m`; text plain. DIFFER. (Box chars present in both because `format_text` already uses a tree.) |
| rank ceremony | FIXED | pretty 2 ANSI vs 0. DIFFER. |
| rank contributors | FIXED | requires `--repos-dir`; with it: pretty 4 ANSI vs 0. DIFFER. (Implementer mislabeled this as "no data here" — it works once the required arg is supplied.) |
| sessions subagents | WIRED (no local data) | `subagents()` (sessions.rs:579) requires a session-ID positional; body calls `self.resolve_format(resolved_root)` (line 593) then renders via `#[cli(display_with = "display_output")]`; `SubagentsReport` has a distinct `format_pretty` (subagents.rs:57). Would dispatch pretty with data. |
| analyze cross_repo_health | WIRED (single-repo = no data) | `cross_repo_health()` (analyze.rs:429) calls `self.resolve_format(...)` (line 435) + `display_output`; `CrossRepoHealthReport::format_pretty` exists (cross_repo_health.rs:79). Would dispatch pretty with multiple repos. |

6/8 confirmed by live output, 2/8 confirmed wired by code (genuinely produce no data in this single-repo checkout).

## 3. Regression check on previously-working commands — PASS (no regressions)

Spot-checked across services (`--pretty` vs default, piped):

- sessions list — DIFFER (21 ANSI) ✓
- rank test-ratio — DIFFER (1 ANSI) ✓
- rank complexity — DIFFER (11 ANSI) ✓
- analyze summary — DIFFER (17 ANSI) ✓
- view <file> — DIFFER (7 ANSI) ✓
- rules list — DIFFER (291 ANSI) ✓

`rank imports` showed SAME / 0 ANSI — **not a regression**: its report `ImportCentralityReport`
(imports.rs) defines only `format_text`, no `format_pretty`, so it never had distinct pretty
output (default-trait fallback). `config show` SAME for the same reason. The method itself
*does* call `resolve_format` correctly. So the "SAME" results are reports without a pretty
variant, which is correct behavior.

## 4. Root-aware resolution — PASS (correct)

Empirically constructed a target dir ≠ cwd with `.normalize/config.toml` `[pretty] enabled = true`:

- `analyze health -r <dir>` **piped** → 15 ANSI present. Proves `resolve_format` loaded the
  **target root's** config, not cwd's. Root-aware. ✓
- Same dir with `enabled = false` + `--pretty` flag → 15 ANSI (flag overrides config), matching
  `resolve_pretty = !compact && (pretty || config.pretty.enabled())`. ✓
- cwd (no pretty config) piped → 0 ANSI (text). ✓

Single-target commands resolve against the command's `root_path` (the `-r`/target), confirmed
in code for rank.rs and analyze.rs.

**Multi-repo commands** (`analyze cross_repo_health`, `rank contributors`) resolve against
`std::env::current_dir()` rather than `repos_dir` (analyze.rs:435, rank.rs contributors body).
Assessment: **acceptable / arguably correct**, not a bug. There is no single "command root" for
a multi-repo scan, and pretty is a display preference tied to the invoking terminal — the user's
cwd config is the natural place to read it, not some scanned target tree. TTY auto-detection is
unaffected (it keys off stdout, not root). The implementer flagged this as a possible
misresolution; for the purpose of *pretty* state it is defensible.

## 5. TTY auto-detect — PASS

`PrettyConfig::enabled()` (normalize-output/src/lib.rs:64) returns the config value if set,
else `std::io::stdout().is_terminal()`. Confirmed by behavior: every command above produced
text (0 ANSI) when piped with no flag, and pretty when `--pretty` was passed. Default-in-TTY /
text-when-piped works.

## 6. ratchet / budget / package --pretty removal — CONCERN (cosmetic / commit message inaccurate)

- Service-level `global = [pretty, compact]` and the dead `pretty` Cell **are removed** from
  all three service files (grep for pretty/compact/global in those files is empty;
  `#[cli]` is bare). The dead-flag cleanup is real and correct.
- **However**, `normalize ratchet --help` / `budget --help` / `package --help` **still show
  `--pretty` / `--compact`**, inherited from the root `NormalizeService` global (which legitimately
  declares them for the many commands that do support pretty). A pretty-capable subcommand
  (`analyze --help`) shows `--pretty` exactly once too — so help is indistinguishable.
- `normalize ratchet check --pretty` runs and is **accepted but inert** (these reports have only
  `format_text`).
- Net: the commit message's claim "drop the `--pretty` advertisement **entirely**" is **inaccurate** —
  the advertisement persists via root-global inheritance and the flag is still a silent no-op for
  these commands. This is **identical to pre-migration behavior** (not a regression) and cannot be
  fixed at the service level given the root global; it would require server-less to suppress
  inherited globals on subcommands that don't honor them. Compiles and runs fine.

## 7. CliGlobals impls — PASS

All 9 `global`-declaring services (mod/`NormalizeService`, analyze, rank, sessions, config, view,
trend, context, rules) implement `CliGlobals`. Every impl is uniform and correct:

```rust
fn set_global_flag(&self, name: &str, value: bool) {
    match name {
        "pretty"  => self.pretty_raw.set(value),
        "compact" => self.compact_raw.set(value),
        _ => {}            // unknown flags ignored — sane
    }
}
```

No impl drops a declared value on the floor → **no re-introduced silent no-op**. `rules` resolves
lazily via `pretty_active()` (root-independent, correct for config-wide rule output). Unknown flag
names are a safe no-op.

## Additional finding (pre-existing, out of scope) — CONCERN

7 `analyze` methods use `display_output` but never call `resolve_format`: `security`, `docs`,
`activity`, `repo_coupling`, `liveness`, `effects`, `exceptions` (analyze.rs). Checked git
`91e32b8f`: these **also lacked** pretty/compact params and `resolve_format` *before* the
migration — so this is **pre-existing inertness, not introduced here**. For any of these whose
report defines a distinct `format_pretty` (e.g. `coupling_clusters` has one and *is* wired;
verify the others case-by-case), `--pretty` would be silently inert and they would not auto-pretty
in a TTY. Same bug *class* as the original 8, but a different, untouched set. Worth a follow-up
TODO; does not block this migration.

## Verdict

Migration is **sound and ready to merge.** The user-visible bug (8 inert `--pretty` commands) is
fixed; no previously-working command regressed; root-aware resolution and TTY detection are
correct; the CliGlobals plumbing is uniform with no dropped values. Two non-blocking CONCERNs:
(a) the commit message overstates the ratchet/budget/package change — `--pretty` still appears in
their help via the root global and remains inert (unchanged from before); (b) a separate,
pre-existing set of 7 analyze methods never resolves pretty and should be a follow-up.
