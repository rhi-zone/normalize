# normalize rules

Manage and run analysis rules (syntax + fact). This is the unified entry point for all rule types — tree-sitter syntax rules (`.scm`) and Datalog fact rules (`.dl`).

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

### list

List installed rules:

```bash
normalize rules list                # All rules
normalize rules list --engine syntax  # Syntax rules only
normalize rules list --engine fact    # Fact rules only
normalize rules list --sources      # Show source URLs
normalize rules list --json
```

Options:
- `--engine <ENGINE>` — Filter by rule engine: `all`, `syntax`, `fact` (default: `all`)
- `--sources` — Show source URLs for imported rules

### run

Run rules against the codebase:

```bash
normalize rules run                         # Run all rules
normalize rules run --engine syntax           # Syntax rules only
normalize rules run --engine fact             # Fact rules only
normalize rules run --rule rust/unwrap-in-impl  # Specific rule
normalize rules run --fix                   # Apply auto-fixes (syntax only)
normalize rules run --sarif                 # SARIF output
normalize rules run src/                    # Target specific path
```

Arguments:
- `[TARGET]` — Target directory or file

Options:
- `--rule <RULE>` — Specific rule ID to run
- `--engine <ENGINE>` — Filter by rule engine: `all`, `syntax`, `fact` (default: `all`)
- `--fix` — Apply auto-fixes (syntax rules only)
- `--sarif` — Output in SARIF format
- `--debug <FLAGS>` — Debug flags (comma-separated)

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
