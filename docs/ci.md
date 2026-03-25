# CI Integration

`normalize ci` is the single entry point for running all normalize checks in CI. It runs all
configured rule engines in sequence, aggregates violations into a unified report, and exits
non-zero if any errors are found.

## Quick Start

Add two steps to any CI pipeline:

```bash
# 1. Install normalize
curl -fsSL https://raw.githubusercontent.com/rhi-zone/normalize/master/install.sh | sh

# 2. Run all checks
normalize ci
```

`normalize ci` exits 0 when there are no errors, exits 1 when errors are found. Warnings
don't fail CI by default — use `--strict` to make them fail too.

If `.normalize/ratchet.json` doesn't exist, the ratchet check is a no-op. Repos that
haven't configured ratchet or budget are not penalized.

## What `normalize ci` Runs

The command runs three engines in sequence:

### 1. Syntax engine (`--no-syntax` to skip)

Tree-sitter-based pattern rules defined in `.scm` query files. Checks code patterns like
unwrapped results, bare excepts, hardcoded secrets, and any custom rules you've added.

```bash
normalize rules list --type syntax    # see which syntax rules are enabled
```

### 2. Native engine (`--no-native` to skip)

Built-in Rust checks that don't fit the `.scm` model:

- **stale-summary**: `SUMMARY.md` files that haven't been updated since recent changes
- **stale-docs**: documentation files referencing removed symbols
- **check-examples**: code examples in docs that no longer parse
- **check-refs**: cross-file references that point to missing targets
- **ratchet**: metric regression check against `.normalize/ratchet.json` baseline
- **budget**: diff-based budget check against `.normalize/budget.json`

### 3. Fact engine (`--no-fact` to skip)

Datalog-style rules that reason across the full codebase graph (imports, calls, symbols).
Used for cross-file checks like circular dependency detection.

```bash
normalize rules list --type fact      # see which fact rules are enabled
```

## GitHub Actions

Complete working example with version pinning and binary install:

```yaml
name: normalize

on:
  push:
    branches: [main]
  pull_request:

jobs:
  normalize:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Install normalize
        run: |
          curl -fsSL https://raw.githubusercontent.com/rhi-zone/normalize/master/install.sh | sh
        env:
          NORMALIZE_VERSION: "0.2.0"
          INSTALL_DIR: /usr/local/bin

      - name: Run normalize ci
        run: normalize ci

      # Optional: SARIF upload for inline PR annotations
      - name: Generate SARIF
        if: always()
        run: normalize ci --sarif > normalize.sarif || true

      - name: Upload SARIF
        if: always()
        uses: github/codeql-action/upload-sarif@v3
        with:
          sarif_file: normalize.sarif
        continue-on-error: true
```

For SARIF annotations on PRs, run `normalize ci --sarif` and upload the output to
GitHub's code scanning API. The `--sarif` flag outputs SARIF 2.1.0 JSON to stdout.
Generate the SARIF file before uploading it.

### Opting out of engines

```yaml
- name: Run normalize ci (syntax only)
  run: normalize ci --no-native --no-fact
```

## GitLab CI

```yaml
normalize:
  image: ubuntu:latest
  stage: test
  before_script:
    - apt-get update -qq && apt-get install -y -qq curl
    - curl -fsSL https://raw.githubusercontent.com/rhi-zone/normalize/master/install.sh | sh
  script:
    - normalize ci
  artifacts:
    when: always
    reports:
      codequality: normalize.json
  after_script:
    - normalize ci --json > normalize.json || true
```

To pin the version:

```yaml
  variables:
    NORMALIZE_VERSION: "0.2.0"
    INSTALL_DIR: /usr/local/bin
```

## Configuring for Your Repo

### Severity overrides

Override rule severity in `.normalize/config.toml`:

```toml
[rules."rust/unwrap-in-impl"]
severity = "warning"   # downgrade from error

[rules."stale-summary"]
severity = "error"     # upgrade to error (default is warning)

[rules."python/bare-except"]
enabled = false        # disable entirely
```

### Enabling/disabling rules

```bash
normalize rules enable python/bare-except    # enable a rule
normalize rules disable no-todo-comment      # disable a rule
normalize rules enable --tag correctness     # enable a tag group
```

### CI-only severity

To apply stricter rules only in CI, use the `--strict` flag:

```yaml
- run: normalize ci --strict    # warnings also fail CI
```

## Ratchet + Budget Workflow

Ratchet prevents metric regressions (complexity creep, test ratio decline, etc.).
Budget limits how much a metric can change in a single diff.

### Bootstrap ratchet

```bash
# Pin current baselines for all Rust files
normalize ratchet add src/ --metric complexity

# Commit the baseline
git add .normalize/ratchet.json
git commit -m "chore: add normalize ratchet baselines"
```

CI then runs `normalize ci` and catches regressions automatically.

### When you intentionally raise a value

```bash
# Accept the new (higher) value as the new baseline
normalize ratchet update src/big_module.rs --metric complexity --force

# Commit the updated baseline alongside your change
git add .normalize/ratchet.json
git commit -m "refactor: increase complexity baseline for big_module"
```

### Budget example

```bash
# Limit complexity growth to +10 per PR
normalize budget add src/ --metric complexity --limit 10

git add .normalize/budget.json
git commit -m "chore: add complexity budget"
```

See `normalize ratchet --help` and `normalize budget --help` for the full API.

## Pinning the Version

Pin to a specific version for reproducible CI:

```bash
# Via install script (fast — downloads a prebuilt binary)
NORMALIZE_VERSION=0.2.0 curl -fsSL https://raw.githubusercontent.com/rhi-zone/normalize/master/install.sh | sh

# Via cargo (slower — compiles from source)
cargo install normalize --version "0.2.0" --locked
```

Check for new versions with:

```bash
normalize update --check
```

## Troubleshooting

**Index not built:** Fact rules require the index. Run `normalize structure rebuild` before
`normalize ci`, or skip the fact engine with `--no-fact` if you haven't set up the index yet.

**No config file:** If `.normalize/config.toml` doesn't exist, normalize uses built-in defaults.
Run `normalize init` to generate a config file with commented-out options.

**Rules not finding violations:** Verify grammars are installed (`normalize grammars list`).
Syntax rules require the tree-sitter grammar for the target language. If the grammar is missing,
those rules silently produce no results.

**SHA256 mismatch on install:** The install script downloads `SHA256SUMS.txt` from the same
release and verifies the archive before installing. A mismatch means the download was corrupted
or the release assets don't match — retry the download. Do not skip verification.
