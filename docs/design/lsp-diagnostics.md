# LSP Diagnostics from Rule Engines

Status: **in progress**

## Problem

`normalize serve lsp` provides symbols, hover, definition, references, and rename — but no diagnostics. Rule engines (syntax rules, fact rules) produce `DiagnosticsReport` with `Issue` structs that map directly to LSP `Diagnostic`, but the LSP server doesn't run them.

## Design

### Issue → LSP Diagnostic mapping

Fields align directly:

| Issue field | LSP Diagnostic field |
|-------------|---------------------|
| `file` | URI (group diagnostics by file) |
| `line`, `column`, `end_line`, `end_column` | `range` |
| `severity` | `severity` (Error→1, Warning→2, Info→3, Hint→4) |
| `message` | `message` |
| `rule_id` | `code` |
| `source` | `source` (e.g. "normalize/syntax-rules") |
| `related` | `relatedInformation` |

### Architecture

```
did_save notification
  → debounce (500ms)
  → spawn background task
  → run_rules_report(root, ...) on workspace
  → group Issues by file
  → publish_diagnostics per file
```

### Approach: on-save with debounce

1. **`did_save`** triggers diagnostics run (not `did_change` — rules work on disk content)
2. **Debounce**: 500ms after last save, run once. Cancels pending runs.
3. **Background**: rules run on a background tokio task, don't block LSP responses
4. **Per-file publishing**: group issues by `file`, call `client.publish_diagnostics()` for each
5. **Clear stale**: when re-running, publish empty diagnostics for files that previously had issues but no longer do

### Why not incremental?

- Syntax rules scan whole files (tree-sitter parse + pattern match) — file-level is the natural granularity
- Fact rules need full index — no incremental path exists
- A full run on save is fast enough for most projects (syntax rules: <1s for ~1000 files)

### State needed in MossBackend

```rust
/// Files that had diagnostics in the last run (to clear stale ones).
diagnosed_files: Mutex<HashSet<Url>>,
/// Cancel token for debounced runs.
diagnostics_cancel: Mutex<Option<tokio::sync::watch::Sender<()>>>,
```

### Implementation steps

1. Add `did_save` handler to `MossBackend`
2. Add `issue_to_lsp_diagnostic()` conversion function
3. Run `run_rules_report()` in background task after debounce
4. Publish diagnostics per file, clear stale files
5. Add `diagnosed_files` tracking to clear removed issues

### Config

No new config needed initially. The existing `normalize.toml` rule configuration (`analyze.rules`, `analyze.facts_rules`) applies. The LSP server reads it the same way the CLI does.

### Future: per-file syntax rules

For syntax rules specifically, we could run only on the saved file (not the whole workspace). This would be a performance optimization for large repos — run workspace-wide on startup, then per-file on save.
