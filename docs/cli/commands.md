# CLI Commands

## moss synthesize

Synthesize code from a specification.

```bash
moss synthesize <description> [options]
```

### Arguments

| Argument | Description |
|----------|-------------|
| `description` | Natural language description of what to synthesize |

### Options

| Option | Description |
|--------|-------------|
| `--type`, `-t` | Type signature (e.g., `"(int, int) -> int"`) |
| `--example`, `-e` | Input/output example (can be repeated) |
| `--constraint`, `-c` | Constraint to satisfy (can be repeated) |
| `--test` | Test code to validate against (can be repeated) |
| `--dry-run` | Show decomposition without synthesizing |
| `--show-decomposition` | Show subproblems during synthesis |
| `--preset` | Configuration preset (default/research/production/minimal) |
| `--json` | Output as JSON |
| `--verbose`, `-v` | Verbose output |

### Examples

```bash
# Basic synthesis
moss synthesize "Create a function that adds two numbers"

# With type signature
moss synthesize "Sort a list" --type "List[int] -> List[int]"

# With examples
moss synthesize "Reverse a string" \
    --example "hello" "olleh" \
    --example "world" "dlrow"

# Dry run to see decomposition
moss synthesize "Build a REST API for users" --dry-run

# JSON output
moss synthesize "Add numbers" --json | jq .code
```

### Presets

| Preset | Description |
|--------|-------------|
| `default` | Balanced settings |
| `research` | More iterations, deeper search |
| `production` | Strict validation, conservative |
| `minimal` | Fast, shallow search |

```bash
moss synthesize "Complex task" --preset research
```

## moss edit

Apply structural edits to code (coming soon).

```bash
moss edit <file> <instruction>
```

## moss view

Extract structural views from code (coming soon).

```bash
moss view <file> [--skeleton|--cfg|--deps]
```

## moss summarize

Generate a hierarchical summary of a codebase.

```bash
moss summarize [directory] [options]
```

### Options

| Option | Description |
|--------|-------------|
| `--include-private`, `-p` | Include private (_prefixed) modules and symbols |
| `--include-tests`, `-t` | Include test files |
| `--docs`, `-d` | Summarize documentation files instead of code |
| `--json`, `-j` | Output as JSON |

### Examples

```bash
# Summarize current directory
moss summarize

# Summarize specific project
moss summarize ~/projects/myapp

# Include everything
moss summarize --include-private --include-tests

# Summarize documentation instead of code
moss summarize --docs

# Get JSON for further processing
moss summarize --json | jq .stats
```

## moss check-docs

Verify documentation freshness against the codebase.

```bash
moss check-docs [directory] [options]
```

### Options

| Option | Description |
|--------|-------------|
| `--strict`, `-s` | Exit with error on warnings (not just errors) |
| `--check-links`, `-l` | Check for broken internal links |
| `--json`, `-j` | Output as JSON |

### What it checks

- **Stale references**: Documentation mentions code that doesn't exist
- **Missing documentation**: Code not mentioned in docs
- **Outdated statistics**: Line counts in README don't match reality
- **Broken links** (with `-l`): Internal links that point to non-existent files

### Examples

```bash
# Check current project
moss check-docs

# Strict mode for CI
moss check-docs --strict

# Include link verification
moss check-docs --check-links

# Get structured output
moss check-docs --json | jq .stats.coverage
```

## moss check-todos

Verify TODO.md accuracy against implementation and code comments.

```bash
moss check-todos [directory] [options]
```

### Options

| Option | Description |
|--------|-------------|
| `--strict`, `-s` | Exit with error on orphaned TODOs |
| `--json`, `-j` | Output as JSON |

### What it checks

- **Tracked items**: Checkbox items in TODO.md with status
- **Code TODOs**: TODO/FIXME/HACK/XXX comments in source
- **Orphaned TODOs**: Code TODOs not tracked in TODO.md
- **Categories**: Groups items by markdown headers

### Examples

```bash
# Check current project
moss check-todos

# Strict mode for CI
moss check-todos --strict

# Get completion stats
moss check-todos --json | jq .stats
```

## Environment Variables

| Variable | Description |
|----------|-------------|
| `MOSS_CONFIG` | Path to config file (default: `moss.toml`) |
| `MOSS_LOG_LEVEL` | Logging level (DEBUG, INFO, WARNING, ERROR) |
| `ANTHROPIC_API_KEY` | API key for Anthropic LLM |
| `OPENAI_API_KEY` | API key for OpenAI LLM |

## Configuration File

Create `moss.toml` in your project root:

```toml
[synthesis]
max_depth = 5
max_iterations = 50
parallel_subproblems = true

[synthesis.generators]
enabled = ["template", "llm"]

[synthesis.llm]
provider = "anthropic"
model = "claude-sonnet-4-20250514"
```

## Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Success |
| 1 | Synthesis failed |
| 2 | Invalid arguments |
| 3 | Configuration error |
