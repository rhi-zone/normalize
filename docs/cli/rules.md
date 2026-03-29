# normalize rules

Manage and run analysis rules (syntax + fact + native). This is the unified entry point for all rule engines — tree-sitter syntax rules (`.scm`), Datalog fact rules (`.dl`), and native checks (stale-summary, check-refs, ratchet, budget).

**Command path:** `normalize rules <subcommand>`

## Subcommands

| Subcommand | Description |
|------------|-------------|
| `list` | List all rules (syntax + fact, builtin + user) |
| `run` | Run rules against the codebase |
| `enable` | Enable a rule or all rules matching a tag |
| `disable` | Disable a rule or all rules matching a tag |
| `show` | Show full documentation for a rule |
| `tags` | List all tags and the rules they group |
| `add` | Add a rule from a URL |
| `update` | Update imported rules from their sources |
| `remove` | Remove an imported rule |
| `setup` | Interactive setup wizard — run all rules and walk through enable/disable |
| `validate` | Validate rule configuration for errors |

### list

List installed rules:

```bash
normalize rules list                # All rules
normalize rules list --type syntax  # Syntax rules only
normalize rules list --type fact    # Fact rules only
normalize rules list --json
```

Options:
- `-t, --type <TYPE>` — Filter by rule type: `all`, `syntax`, `fact` (default: `all`)

### run

Run rules against the codebase:

```bash
normalize rules run                              # Run all rules
normalize rules run --type syntax                # Syntax rules only
normalize rules run --type fact                  # Fact rules only
normalize rules run --type native                # Native checks only (stale-summary, ratchet, budget)
normalize rules run --rule rust/unwrap-in-impl   # Specific rule
normalize rules run --fix                        # Apply auto-fixes (syntax only)
normalize rules run --sarif                      # SARIF output
normalize rules run src/                         # Target specific path
normalize rules run --files src/main.rs src/lib.rs  # Explicit file list (bypasses walker)
normalize rules run --files src/lib.rs --only "*.rs" # Compose with filters
```

Arguments:
- `[TARGET]` — Target directory or file

Options:
- `--rule <RULE>` — Specific rule ID to run
- `-t, --type <TYPE>` — Filter by rule type: `all`, `syntax`, `fact`, `native`, `sarif` (default: `all`)
- `--fix` — Apply auto-fixes (syntax rules only)
- `--sarif` — Output in SARIF 2.1.0 format (for IDE/CI integration)
- `--no-fail` — Exit 0 even when error-severity issues are found
- `--pretty` — Colored terminal output
- `--compact` — Plain text output (default)
- `--json` — JSON output (automatic from `DiagnosticsReport`)
- `--debug <FLAGS>` — Debug flags (comma-separated)
- `--only <GLOB>` — Only include files matching glob patterns
- `--exclude <GLOB>` — Exclude files matching glob patterns
- `--files <PATH>...` — Explicit file paths to check (bypasses file walker; for hook-grade latency)

Output goes through `DiagnosticsReport`, so all standard output formats (`--json`, `--jsonl`, `--jq`, `--schema`, `--pretty`, `--sarif`) work consistently. Both syntax and fact engine findings are merged, sorted by file/line/severity, and rendered through the same pipeline.

### add

Add a rule from a URL. Supports both `.scm` (syntax) and `.dl` (fact) files:

```bash
normalize rules add https://example.com/rules/no-console-log.scm
normalize rules add https://example.com/rules/circular-deps.dl
normalize rules add https://example.com/rules/security.scm --global
```

Options:
- `--global` — Install to global rules (`~/.config/normalize/rules/`) instead of project

The rule file must have TOML frontmatter with an `id` field. Syntax rules use `.scm`:

```scheme
# ---
# id = "no-console-log"
# severity = "warning"
# message = "Avoid console.log in production code"
# ---

(call_expression
  function: (member_expression
    object: (identifier) @obj
    property: (property_identifier) @prop)
  (#eq? @obj "console")
  (#eq? @prop "log")) @match
```

Fact rules use `.dl`:

```datalog
# ---
# id = "too-many-imports"
# message = "File imports more than 20 modules"
# ---

relation import_count(String, i32);
import_count(file, c) <-- import(file, _, _), agg c = count() in import(file, _, _);
warning("too-many-imports", file) <-- import_count(file, c), if c > 20;
```

### enable

Enable a rule or all rules matching a tag:

```bash
normalize rules enable rust/unwrap-in-impl    # Enable specific rule
normalize rules enable --tag security          # Enable all rules tagged "security"
```

### disable

Disable a rule or all rules matching a tag:

```bash
normalize rules disable rust/unwrap-in-impl   # Disable specific rule
normalize rules disable --tag style            # Disable all rules tagged "style"
```

### show

Show full documentation for a rule:

```bash
normalize rules show rust/unwrap-in-impl
```

### tags

List all tags and the rules they group:

```bash
normalize rules tags
```

### update

Update imported rules from their source URLs:

```bash
normalize rules update              # Update all imported rules
normalize rules update no-console-log  # Update specific rule
```

Only rules with tracked sources (added via URL) will be updated. Local rules are skipped.

### remove

Remove an imported rule:

```bash
normalize rules remove no-console-log
```

This removes both the rule file and its entry in the lock file.

### setup

Interactive wizard that runs all rules against the codebase, groups violations by rule, and walks you through each rule — showing example violations and prompting to enable or disable. Recommended rules (correctness, security, bug-prone) are shown first.

```bash
normalize rules setup                  # Interactive rule configuration
normalize rules setup --root /path     # Run against a specific project
```

This is the same wizard available via `normalize init --setup`, but can be run standalone at any time without re-initializing.

### validate

Validate the rules configuration — check rule IDs, TOML syntax, and report issues:

```bash
normalize rules validate
```

## Lock File

Imported rules are tracked in `.normalize/rules.lock` (project) or `~/.config/normalize/rules.lock` (global):

```toml
[rules.no-console-log]
source = "https://example.com/rules/no-console-log.scm"
sha256 = "abc123..."
added = "2024-01-15"
```

## See Also

- [Syntax Rules Writing Guide](../syntax-rules.md) — Create `.scm` rules with tree-sitter queries
- [Fact Rules Writing Guide](../fact-rules.md) — Create `.dl` rules with Datalog
- [analyze](analyze.md) — Run analysis with rules
