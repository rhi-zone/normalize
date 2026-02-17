# normalize rules

Manage and run analysis rules (syntax + fact). This is the unified entry point for all rule types — tree-sitter syntax rules (`.scm`) and Datalog fact rules (`.dl`).

## Subcommands

| Subcommand | Description |
|------------|-------------|
| `list` | List all rules (syntax + fact, builtin + user) |
| `run` | Run rules against the codebase |
| `add` | Add a rule from a URL |
| `update` | Update imported rules from their sources |
| `remove` | Remove an imported rule |

### list

List installed rules:

```bash
normalize rules list                # All rules
normalize rules list --type syntax  # Syntax rules only
normalize rules list --type fact    # Fact rules only
normalize rules list --sources      # Show source URLs
normalize rules list --json
```

Options:
- `--type <TYPE>` — Filter by rule type: `all`, `syntax`, `fact` (default: `all`)
- `--sources` — Show source URLs for imported rules

### run

Run rules against the codebase:

```bash
normalize rules run                         # Run all rules
normalize rules run --type syntax           # Syntax rules only
normalize rules run --type fact             # Fact rules only
normalize rules run --rule rust/unwrap-in-impl  # Specific rule
normalize rules run --fix                   # Apply auto-fixes (syntax only)
normalize rules run --sarif                 # SARIF output
normalize rules run src/                    # Target specific path
```

Arguments:
- `[TARGET]` — Target directory or file

Options:
- `--rule <RULE>` — Specific rule ID to run
- `--type <TYPE>` — Filter by rule type: `all`, `syntax`, `fact` (default: `all`)
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

- [Syntax Rules Writing Guide](../rules.md) — Create `.scm` rules with tree-sitter queries
- [Fact Rules Writing Guide](../fact-rules.md) — Create `.dl` rules with Datalog
- [facts](facts.md) — `normalize facts` command (lower-level fact rule execution)
- [analyze](analyze.md) — Run analysis with rules
