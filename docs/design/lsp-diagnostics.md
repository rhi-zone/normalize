# LSP Diagnostics from Rule Engines

Status: **implemented**

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

### Architecture: two-tier diagnostics

```
did_save(uri)
├── run_syntax_diagnostics(uri)          # immediate, per-file
│   ├── run_rules_report(file, root, Syntax)  # ~10-50ms
│   └── publish syntax diagnostics for file
│
└── schedule_fact_diagnostics()           # debounced 1500ms
    ├── update_file(rel_path) on index    # incremental reindex
    ├── run_rules_report(root, root, Fact)# full Datalog on fresh index
    └── publish fact diagnostics for all files
```

### Tier 1: per-file syntax diagnostics (immediate)

1. **`did_save`** triggers per-file syntax rules immediately
2. Runs `run_rules_report(file_path, root, RuleType::Syntax)` — only syntax rules on the saved file
3. Publishes diagnostics for that single file, tracking syntax-diagnosed files separately
4. Fast: ~10-50ms for a single file

### Tier 2: workspace-wide fact diagnostics (debounced)

1. **`did_save`** also schedules fact diagnostics with 1500ms debounce
2. Before running fact rules, incrementally updates the index via `FileIndex::update_file()`
3. Runs `run_rules_report(root, root, RuleType::Fact)` on the full workspace
4. Publishes fact diagnostics workspace-wide, tracking fact-diagnosed files separately

### Diagnostic source separation

Issues have a `source` field (`"syntax-rules"` or `"fact-rules"`). Each tier only replaces diagnostics from its own source:
- Syntax tier replaces `normalize/syntax-rules` diagnostics for the saved file
- Fact tier replaces `normalize/fact-rules` diagnostics workspace-wide

### State in NormalizeBackend

```rust
syntax_diagnosed_files: Arc<Mutex<HashSet<Url>>>,
fact_diagnosed_files: Arc<Mutex<HashSet<Url>>>,
fact_diagnostics_generation: Arc<AtomicU64>,
```

### Config

No new config needed. The existing `normalize.toml` rule configuration (`rules`) applies.
