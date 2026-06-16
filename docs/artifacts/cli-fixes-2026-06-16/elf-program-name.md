# Investigation: `normalize.elf` in Usage Strings

**Date:** 2026-06-16
**Symptom:** `Usage: normalize.elf rank [OPTIONS]` — clap prints the binary name as `normalize.elf`.

---

## Root Cause

Three facts combine to produce the bug:

1. **The installed artifact is `runtime/normalize.elf`** — the release workflow
   (`release.yml`, line 169) copies the built binary to `dist/runtime/normalize.elf`.
   The actual ELF on disk at `~/.local/share/normalize/runtime/normalize.elf` has
   `.elf` as a real file extension, not a description.

2. **The wrapper script passes the ELF path as `argv[0]`** — the POSIX shell wrapper
   (`~/.local/share/normalize/normalize`, also inlined in `release.yml` lines 211–238)
   invokes the musl loader like this:

   ```sh
   exec "$RUNTIME/ld-musl-x86_64.so.1" \
       --library-path "$RUNTIME" \
       "$RUNTIME/normalize.elf" "$@"
   ```

   When `exec` replaces the shell with the musl loader, the loader sets the ELF
   binary's `argv[0]` to the third argument — `$RUNTIME/normalize.elf`. The
   user's arguments (`"$@"`) follow; there is no explicit `argv[0]` override.
   The musl loader (version 1.2.4) does not expose an `--argv0` option.

3. **`main.rs` passes the raw `argv` (including `argv[0]`) to clap** — `main.rs`
   collects `std::env::args_os()` into `argv` (line 152) and passes the whole
   slice — `argv[0]` included — to `cli_run_with_async(argv)` (line 226). Inside
   server-less, `cli_run_with_async` calls `get_matches_from(args)`. Clap's
   `try_get_matches_from_mut` reads the first element as the program name using
   `Path::file_name()` (clap_builder 4.6.0, `command.rs` line 902), which yields
   `"normalize.elf"` rather than the stem `"normalize"`.

   Note: `main.rs` does compute `argv0` via `file_stem()` at line 154–158, which
   correctly produces `"normalize"` — but this is only used for the symlink dispatch
   (`jq`, `rg`, `ast-grep`), not passed to clap.

---

## Why `.elf`?

`Path::file_name()` (used by clap) returns the full filename including extension:
`normalize.elf`. `Path::file_stem()` (used in main.rs for dispatch) strips the
extension and returns `normalize`. Clap uses `file_name()`, so it sees the full
`normalize.elf`.

---

## Which Repo Owns the Fix?

**This is a normalize-repo issue.** The bug is in the wrapper script and/or `main.rs`
— not in server-less. Server-less does the right thing: it passes the first element
of the provided args to clap as-is, which is correct behavior for `cli_run_with`. The
problem is what normalize passes as `argv[0]`.

---

## Fix Locations

Two independent fix sites, either of which resolves the symptom:

### Option A — Rewrite `argv[0]` in `main.rs` (preferred)

**File:** `crates/normalize/src/main.rs`

Before passing `argv` to `cli_run_with_async`, replace `argv[0]` with the
canonical name. `argv0` (already computed via `file_stem()`) is `"normalize"` in
both the debug binary and the installed release binary. Replace:

```rust
let argv: Vec<std::ffi::OsString> = std::env::args_os().collect();
```

with something like:

```rust
let mut argv: Vec<std::ffi::OsString> = std::env::args_os().collect();
// Normalize argv[0] to the canonical program name so clap usage strings
// print "normalize" regardless of the on-disk filename (e.g. normalize.elf).
if let Some(first) = argv.first_mut() {
    let stem = std::path::Path::new(&*first)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("normalize");
    *first = stem.into();
}
```

This is minimal, self-contained, and doesn't require changes to server-less or
the release workflow. The `argv0` variable computed immediately after (lines 154–158)
would then also trivially equal the stem (now explicit in `argv[0]`).

### Option B — Fix the wrapper script to set argv0 explicitly

**File:** `.github/workflows/release.yml` (lines 235–237, the inlined `WRAPPER` heredoc)
**Synced copy:** `~/.local/share/normalize/normalize` (installed wrapper)

Replace the `exec` invocation with one that passes the wrapper name as `argv[0]`.
POSIX `sh` does not support `exec -a`, but GNU bash and most modern `/bin/sh`
implementations do. Alternatively, call a small C helper, use `/proc/self/exe`,
or use `busybox env -a`. This is fragile across platforms and musl loader versions.

Option A is simpler and more robust.

---

## Summary

The ELF binary is literally named `normalize.elf` on disk; the wrapper invokes it
via the musl loader without overriding `argv[0]`; and clap reads `file_name()` of
`argv[0]` to derive the usage name, which gives `normalize.elf`. Fix in
`crates/normalize/src/main.rs` by rewriting `argv[0]` to its `file_stem()` before
passing to `cli_run_with_async`.
