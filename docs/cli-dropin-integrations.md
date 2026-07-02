# Embedded CLI Drop-in Integrations

> Three integrations complete: `jq` (via jaq), `rg` (vendored ripgrep 15.1.0), `ast-grep`/`sg` (via ast-grep-core + normalize-languages). ast-grep has near-full parity: run, scan, test with rewrite/interactive support.

normalize embeds several tools as library dependencies. Rather than making users install
those tools separately, we expose them as drop-in CLI replacements — both as subcommands
(`normalize jq`) and via argv[0] symlink dispatch (`jq -> normalize`).

## Why not subprocess wrappers?

The tools are already linked into the binary. A subprocess wrapper would:
- Require the user to have the tool installed anyway
- Add process spawn overhead
- Duplicate the binary on disk

## Why not server-less?

Vendored CLIs bypass server-less entirely. Reasons:
- Each tool has its own established flag conventions (`-r`, `-c`, `-n`, etc.)
- Exit code semantics differ (jq uses exit code 2 for usage errors, 1 for false output with `-e`)
- `--help` and `--version` output should match the original tool
- server-less would add `--json`/`--jq`/`--schema` flags that don't belong on a jq clone

As a result, vendored CLI subcommands **do not appear in `normalize --help`**. They are
undocumented from server-less's perspective, documented only here and in their own `--help`.

## Why the line count is not the cost

The vendored CLI front-ends are large by line count — roughly 21.6k lines in the main
crate: `src/rg/` ~13.9k (of which `rg/flags/defs.rs` alone is ~7.8k of ripgrep flag
definitions), `src/ast_grep/` ~7.0k, `src/jq/` ~0.7k. A naive reading treats that ~26% of
the main crate as expensive weight worth reclaiming. It isn't — line count is a poor proxy
for cost here.

The real cost is the **engine libraries**, and they are already paid for:

- **The engines are sunk cost.** `jaq-core`/`-std`/`-json`, `grep`/`grep-*`/`ignore`, and
  `ast-grep-core`/`-config` are already linked into the binary because normalize's
  *first-class* features require them — `--jq` compiles jaq via `jaq_core`, `normalize grep`
  drives `grep`/`ignore` directly, and ast-grep functionality runs through
  `normalize_syntax_rules`. That heavy dependency weight is present whether or not we vendor
  the CLIs.
- **The marginal cost of the front-ends is near-zero.** The vendored CLIs are a *parallel*
  path: they have no callers outside their own directories except the handful of
  argv[0]/subcommand dispatch lines in `main.rs`. The ~21.6k lines are mostly declarative
  flag tables sitting on top of an engine that is already linked. They add negligible
  binary, compile, and dependency weight beyond the engines themselves (see the size table
  below — the jq feature measured ~835 KB, most of it monomorphization noise, not new deps).
- **The marginal benefit is real.** On a system without `jq`/`rg`/`sg` — exactly the bare
  NixOS/container/CI environments normalize targets — you get a genuine, full-interface tool
  essentially for free, because the engine was going to be carried anyway.

So: high benefit ÷ ~zero marginal cost. The economic case for vendoring the *full* CLI
front-ends (not just embedding the engines) rests on this marginal-cost argument, which the
raw line count hides.

### What this argument does *not* settle

The marginal-cost argument justifies that the vendored CLIs are **cheap to keep**. It does
**not** dismiss the size question on other grounds:

- **SRP / encapsulation concerns remain live.** The main crate being the home for ~21.6k
  lines of verbatim third-party tool front-ends is still a single-responsibility and
  encapsulation smell, independent of the marginal binary cost.

The remaining question — whether that smell warrants **extraction into separate crates** —
was considered and found **impossible** (2026-07-02), not merely inadvisable. The vendored
CLIs stay in the main crate because they are held there by a genuine trilemma.

**The trilemma — you cannot have all three:**

1. **Purity** — the vendored third-party CLI *source* lives outside the `normalize` crate.
2. **Publishable-with-drop-ins** — `cargo install normalize` ships the `rg`/`jq`/`sg`
   drop-ins, i.e. the *published* `normalize` reaches the vendored code.
3. **No junk crates** — don't publish verbatim third-party CLI copies to crates.io.

The vendored code is third-party CLI *source* (flag parsing / output / `--help` text) that
upstream **does not publish as a library**: ripgrep/jaq/ast-grep publish their *engines*
(`grep`/`ignore`, `jaq-core`, `ast-grep-core`) but their *CLI front-ends* only as binaries —
which is why the CLI source had to be copied in the first place. So for a *published*
`normalize` to carry the drop-ins, that source must live either (i) inside the `normalize`
crate (violates Purity) or (ii) in a published crate `normalize` depends on (violates
No-junk-crates — it is a verbatim third-party CLI copy on crates.io).

Extracting to `publish = false` crates does not escape it: **a published crate cannot depend
on a `publish = false` crate.** A path-only dependency fails `cargo package`'s "dependency
must have a version" check; a versioned dependency fails registry validation because the
crate isn't published (both verified 2026-07-02). A `publish = false` *multitool binary*
crate resolves the dependency problem but sacrifices (2) — `cargo install normalize` would no
longer carry the drop-ins.

The project has chosen (2) + (3), which **forecloses (1).** Keeping the vendored CLIs in the
main crate is therefore **forced, not merely preferred.**

**Secondary (weaker) point:** a vendored-CLI crate would also fail CLAUDE.md's
crate-existence bar — one dependent (`normalize`), zero standalone value (a verbatim copy of
an already-published upstream tool), and no coherent `normalize-*` name (you cannot republish
ripgrep under a renamed crate). This was the earlier framing; it holds, but the trilemma is
the stronger reason.

This moots the two conditions previously listed as gating extraction (engine version
lockstep, and the publishing-appropriateness question) — there is no reachable extraction to
gate. The SRP smell is mitigated in place: the code lives in isolated module subtrees
(`src/rg/`, `src/ast_grep/`, `src/jq/`) behind capability feature gates
(`cli-full` / `jq-cli` / `rg-cli` / `ast-grep-cli`). Full reasoning:
`docs/audit-2026-07-02.md` ("Decision (2026-07-02): keeping the vendored CLIs in main is
FORCED by a publish trilemma").

## Integration pattern

### 1. Feature gate

Add a `<name>-cli` feature to `crates/normalize/Cargo.toml`, enabled by default:

```toml
[features]
default = ["cli", "jq-cli"]
jq-cli = []
```

This allows size-sensitive builds to opt out. The underlying library deps stay unconditional
(they're pulled in by server-less anyway).

### 2. Vendor the CLI source

Copy the tool's CLI parsing/dispatch code into `src/<tool>/`, with the original license
header. Typically 3–4 files:

```
src/jq/
  cli.rs      # argument parsing
  filter.rs   # compile + run
  mod.rs      # entry point (run_jq)
  help.txt    # --help output
```

Adapt as needed — remove features that depend on deps we don't want to pull in (e.g. we
dropped `jaq-fmts` format support since vanilla jq is JSON-only).

### 3. Gate the module in lib.rs

```rust
#[cfg(feature = "jq-cli")]
pub mod jq;
```

### 4. Dispatch in main.rs

Before server-less runs, check argv[0] and the first subcommand argument:

```rust
#[cfg(feature = "jq-cli")]
{
    let argv0 = argv
        .first()
        .and_then(|p| std::path::Path::new(p).file_stem())
        .and_then(|n| n.to_str())
        .unwrap_or("");
    if argv0 == "jq" {
        return normalize::jq::run_jq(argv[1..].iter().cloned());
    }
    if argv.get(1).and_then(|s| s.to_str()) == Some("jq") {
        return normalize::jq::run_jq(argv[2..].iter().cloned());
    }
}
```

The argv[0] path enables `ln -s normalize jq` symlinks to work transparently.

### 5. Entry point signature

```rust
pub fn run_jq(args: impl Iterator<Item = OsString>) -> ExitCode
```

Takes args *without* argv[0]. Returns `ExitCode` so main.rs can return it directly.

## Binary size findings (jq, 2026-03)

These numbers are from the first integration and inform expectations for future ones.

| Build | Size |
|---|---|
| Baseline (server-less alpha.3, jaq v2) | ~30.0 MB |
| After jaq v2 → v3 upgrade (server-less alpha.4) | ~32.1 MB |
| + jq-cli feature | ~32.9 MB |
| + rg-cli (vendored ripgrep 14.1.1) | ~34.0 MB |
| + ast-grep-cli (ast-grep-core bridge) | ~34 MB (minimal delta) |
| + ast-grep scan/test/rewrite/interactive | ~36 MB (+ast-grep-config, crossterm, serde_yaml) |

**jq-cli feature cost: ~835 KB** (measured by building with and without the feature).

This breaks down roughly as:
- ~240 KB: extra jaq_core/jaq_std monomorphizations not deduplicated with server-less's usage
- ~20 KB: actual CLI code (cli.rs, filter.rs, mod.rs)
- ~575 KB: LTO variation / binary layout noise between builds (hard to attribute precisely)

**jaq-std is already in the binary** via server-less (which calls `jaq_std::funs()` and
`jaq_std::defs()` for its `--jq` flag). The 835 KB is not "adding the stdlib" — it's
additional generic instantiations our code creates alongside server-less's.

**For comparison:** the real `libjq.so` is ~430 KB. Our ~614 KB of jaq symbols is ~1.4× the
C implementation. The overhead is from Rust's monomorphized generics vs C + runtime-compiled
jq stdlib. Not worth trying to eliminate — the coupling required to deduplicate with
server-less's instantiations would cost more in maintainability than ~240 KB saves.

**jaq v2 → v3 upgrade cost: ~2.1 MB.** This is the unavoidable cost of server-less
requiring jaq v3. It would have happened regardless of the jq subcommand.

## Integrations

| Tool | Library | Status |
|---|---|---|
| `jq` | jaq (jaq-core + jaq-std + jaq-json) | Done |
| `rg` | Vendored ripgrep 15.1.0 (crates/core/) + grep, lexopt, termcolor | Done |
| `ast-grep` / `sg` | Vendored ast-grep 0.41.0 CLI + ast-grep-core + DynLang bridge | Done |

### rg parity

Full flag parity — vendored from ripgrep 14.1.1 (Unlicense OR MIT). 1564-line `--help`
output. The only missing feature is PCRE2 (pcre2 feature not enabled).

Symlink dispatch: `rg -> normalize` or `normalize rg`.

### ast-grep parity

Vendored from ast-grep 0.41.0 (MIT). Near-full CLI parity with upstream:

- `sg run` — all flags (`--pattern`, `--lang`, `--selector`, `--strictness`,
  `--debug-query`, `--json`, `--files-with-matches`, `--heading`, `--color`, `--context`,
  `--no-ignore`, `--follow`, `--globs`, `--threads`, `--stdin`, `--rewrite`, `--interactive`)
- `sg scan` — project config (sgconfig.yml), rule discovery, multi-rule scanning with
  CombinedScan, unused suppression detection, severity overrides, `--max-results`
- `sg test` — rule verification against YAML test cases, snapshot generation/comparison,
  interactive snapshot review, parallel test execution

Key difference from upstream: uses normalize-languages' `GrammarLoader` instead of
`ast-grep-language`'s embedded grammars (avoids duplicating ~25 tree-sitter grammars).
The `Lang` type replaces upstream's `SgLang`, delegating to `DynLang` + normalize-languages.

Missing features (can be added later):
- Language injection (HTML/Vue/Svelte embedded languages)
- `--format github` / SARIF output (need `serde-sarif`)
- `--debug-query=pattern` (needs `DumpPattern` API from ast-grep-core 0.41.0)

Symlink dispatch: `sg -> normalize`, `ast-grep -> normalize`, or `normalize ast-grep`.

## jq parity gaps

Flags not supported due to jaq limitations:

| Flag | Description |
|---|---|
| `-a`/`--ascii-output` | Escape non-ASCII characters — jaq_json::write::Pp has no ascii mode |
| `--stream` / `--stream-errors` | Streaming parse mode — not in jaq |
| `--seq` | application/json-seq format — not in jaq |

These are accepted silently by real jq but would error as unknown flags in our implementation.
If jaq adds support in a future version, they can be wired up.
