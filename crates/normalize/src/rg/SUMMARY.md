# src/rg

Embedded ripgrep drop-in replacement, vendored from ripgrep 15.1.0 (MIT/Unlicense). Exposes `run_rg(argv)` as the entry point, invoked via `normalize rg [args...]` or an `rg -> normalize` symlink. Submodules: `flags` (full ripgrep flag definitions, parsing, `HiArgs`/`LowArgs`), `haystack` (file traversal), `search` (core search loop), `logger`, `messages`. Handles broken-pipe gracefully. Provides full ripgrep behavior without a separate installation.
