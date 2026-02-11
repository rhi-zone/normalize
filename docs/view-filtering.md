# Filtering Design

Cross-command filtering for view, analyze, and future batch operations.

## Current State

| Filter | view | analyze | edit |
|--------|------|---------|------|
| Symbol kind | `-t, --type` | `--kind` | - |
| Types only | `--types-only` | - | - |
| Private | `--include-private` | - | - |
| Category | - | - | - |
| Glob | - | - | - |

**Inconsistency**: view uses `-t/--type`, analyze uses `--kind`. Should unify.

## Current Filters (view)

```
-t, --type <KIND>       Symbol type: class, function, method, etc.
--types-only            Architectural view (classes, structs, enums, interfaces)
--include-private       Show private symbols (normally hidden by convention)
--focus[=MODULE]        Resolve imports inline at signature level
--resolve-imports       Inline specific imported symbol signatures
```

## Proposed Additions

### Exclude/Only Design Options

**Option A: Globs Only (no categories)**
```bash
normalize view src/ --exclude="*_test*" --exclude="test_*" --exclude="**/tests/**"
normalize view src/ --only="*.rs"
```
- Pro: Simple, no magic, users know globs
- Pro: No DSL creep
- Con: Verbose for common cases (tests = 5+ patterns per language)
- Con: Users must know language-specific test conventions

**Option B: Separate Flags**
```bash
normalize view src/ --exclude-category=tests --exclude-pattern="*.gen.go"
normalize view src/ --only-category=tests
```
- Pro: Explicit, no ambiguity
- Con: Four flags instead of two
- Con: Verbose

**Option C: Sigil/Prefix**
```bash
normalize view src/ --exclude=@tests --exclude="*.gen.go"
normalize view src/ --only=@tests --only="*.rs"
```
- Pro: One flag pair, explicit distinction
- Con: DSL creep (what's next, `#regex:` prefix?)
- Con: Sigil choice is arbitrary (@, :, %)

**Option D: Smart Detection**
```bash
normalize view src/ --exclude=tests --exclude="*.gen.go"
```
- Pro: Clean syntax
- Con: Magic (contains `*?[` → glob, else category)
- Con: What if category name contains special chars? (unlikely but...)
- Con: Implicit behavior surprises users

**Option E: Categories as Aliases** ✓ CHOSEN

Built-in aliases with config override. `@name` expands to glob patterns.

```bash
normalize view src/ --exclude=@tests          # expands to built-in test patterns
normalize view src/ --exclude="*.gen.go"      # literal glob
```

Built-in aliases (language-aware, no config needed):
- `@tests` - test files for detected languages
- `@config` - config files (*.toml, *.yaml, etc.)
- `@build` - build artifacts (target/, dist/, node_modules/)
- `@docs` - documentation (*.md, docs/)
- `@generated` - generated code

Override or extend in config:
```toml
# .normalize/config.toml
[filter.aliases]
tests = ["*_test.*", "my_custom_tests/**"]  # override built-in
vendor = ["vendor/**", "third_party/**"]     # add new alias
config = []                                   # disable built-in (matches nothing)
```

- Pro: Semantic abstraction over language-specific patterns
- Pro: Built-ins work out of box, config optional
- Pro: `@` sigil is explicit ("resolve this name")
- Pro: Empty array disables cleanly
- Con: Sigil adds syntax, but justified

**Option F: Subcommand for Complex Filtering**
```bash
normalize view src/ --exclude="*_test*"       # simple glob only
normalize filter tests | normalize view src/       # piped filter spec (overdesigned?)
```
- Pro: Simple base case, complex cases are explicit
- Con: Overengineered

---

**Decision:** Option E.

The value isn't brevity—it's semantic abstraction over language-specific details. Users say "exclude tests" without knowing Python uses `test_*.py` while Go uses `*_test.go`. Built-in aliases handle this; config allows override.

### Symbol Kind Filter (extend existing)

Current `-t/--type` accepts single value. Extend to:

```
-t, --type <KIND,...>   Filter by symbol kinds (comma-separated)
```

Examples:
```bash
normalize view file.py -t class,function      # Classes and top-level functions
normalize view file.py -t method              # Only methods (inside classes)
```

## Design Decisions

### 1. Filter Precedence

1. `--only` takes precedence (whitelist mode)
2. `--exclude` removes from result (blacklist mode)
3. Multiple `--exclude` values are OR'd (exclude if any match)
4. Multiple `--only` values are OR'd (include if any match)
5. `-t/--type` applies to symbols, not files

### 2. Interaction with Existing Flags

- `--types-only` is sugar for `-t class,struct,enum,interface,type`
- `--include-private` is orthogonal to all filters (controls visibility, not selection)
- `--focus` and `--resolve-imports` work on filtered result

### 3. Output Indication

When filters are active, indicate in output:
```
src/ (filtered: --exclude="*_test*")
├── lib/
├── main.rs
└── api/
    └── ...
```

## Implementation Notes

- Filters apply during tree traversal, not post-processing
- Glob patterns use gitignore-style matching (same as `ignore` crate)

## Cross-Command Unification

### Proposed Shared Flags

These flags should work identically across view, analyze, and future batch commands:

```
-t, --type <KIND,...>   Symbol kind filter (rename analyze's --kind)
--exclude <GLOB>        Exclude matching paths (repeatable)
--only <GLOB>           Include only matching paths (repeatable)
```

(If we go with Option E, add `@name` syntax for config-defined aliases.)

### Command-Specific Behavior

| Command | How filters apply |
|---------|-------------------|
| view | Filters tree nodes before display |
| analyze | Filters files/symbols before analysis |
| edit | N/A (operates on specific target) |
| grep | Could add `--exclude` for file filtering |
| lint | Could add `--exclude` for file filtering |

### Migration

1. Add `--type` alias to analyze's `--kind` (deprecate `--kind`)
2. Add shared filters to view first
3. Propagate to analyze, grep, lint

## Alias Discoverability

LLMs working with a codebase need to know what aliases are available and what they expand to. A user's config may override built-ins or add custom aliases.

**Solution:** `normalize filter aliases` command

```bash
$ normalize filter aliases
Aliases:
  @tests     *_test.go, test_*.py, *_test.rs, ...  (detected: go, python, rust)
  @config    *.toml, *.yaml, *.json, ...
  @build     target/, dist/, node_modules/, ...
  @docs      *.md, docs/
  @generated *.gen.*, *.pb.go, ...
  @vendor    vendor/**, third_party/**  (custom)
```

With config overrides:
```bash
$ normalize filter aliases
Aliases:
  @tests     (disabled)                             # tests = [] in config
  @config    *.toml, *.yaml, *.json, ...
  @build     target/, dist/, node_modules/, ...
  @docs      *.md, docs/
  @generated *.gen.*, *.pb.go, ...
  @vendor    vendor/**, third_party/**  (custom)   # added in config
  @legacy    old_code/**  (custom)                 # added in config
```

The output is merged: shows effective aliases after config is applied. Annotations clarify origin:
- No annotation = built-in (unmodified)
- `(custom)` = defined in config, not a built-in
- `(disabled)` = config set empty array `[]` to disable built-in
- `(overridden)` = config replaced built-in patterns

### Error Handling

- **Unknown alias** → error: `error: unknown alias @typo`
- **Disabled alias** → warning: `warning: @tests is disabled (matches nothing)`

Unknown is likely a typo. Disabled is intentional config—warn but proceed.

This lets LLMs:
1. Check available aliases before suggesting commands
2. See what patterns an alias expands to
3. Discover project-specific aliases

When alias is used, output shows expansion:
```
src/ (filtered: @tests → *_test.go, test_*.py)
├── lib/
├── main.rs
└── ...
```

## Not Included (Too Complex)

- Regex filters (glob is sufficient, regex is overkill)
- Content-based filters (grep exists for that)
- Complex boolean expressions (use multiple commands with jq)
