# Embedded CLI Drop-in Integrations

> **Work in progress.** We have one integration (`jq` via jaq) so patterns here are based on
> a single data point and subject to change as we add `rg` and `ast-grep`.

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

## Planned integrations

| Tool | Library | Status |
|---|---|---|
| `jq` | jaq (jaq-core + jaq-std + jaq-json) | Done |
| `rg` | grep-matcher + grep-regex + grep-searcher | TODO |
| `ast-grep` | ast-grep-core | TODO |

Note: `ripgrep` has no lib target — `normalize rg` will use the existing grep-* workspace
deps directly rather than vendoring ripgrep's CLI source.
