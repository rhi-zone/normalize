# CI Integration

`normalize ci` is the single entry point for running all normalize checks in CI. It runs all
configured rule engines in sequence, aggregates violations into a unified report, and exits
non-zero if any errors are found.

## Quick Start

Add two steps to any CI pipeline:

```yaml
# 1. Install normalize
- run: cargo install normalize

# 2. Run all checks
- run: normalize ci
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

Complete working example with version pinning and Rust caching:

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

      - uses: dtolnay/rust-toolchain@stable

      - uses: Swatinem/rust-cache@v2

      - name: Install normalize
        run: cargo install normalize --version "0.1.0" --locked

      - name: Run normalize ci
        run: normalize ci

      # Optional: SARIF upload for inline PR annotations
      - name: Upload SARIF
        if: always()
        uses: github/codeql-action/upload-sarif@v3
        with:
          sarif_file: normalize.sarif
        continue-on-error: true

      - name: Generate SARIF
        if: always()
        run: normalize ci --sarif > normalize.sarif || true
```

For SARIF annotations on PRs, run `normalize ci --sarif` and upload the output to
GitHub's code scanning API. The `--sarif` flag outputs SARIF 2.1.0 JSON to stdout.

### Opting out of engines

```yaml
- name: Run normalize ci (syntax only)
  run: normalize ci --no-native --no-fact
```

## GitLab CI

```yaml
normalize:
  image: rust:latest
  stage: test
  cache:
    key: normalize-$CI_COMMIT_REF_SLUG
    paths:
      - ~/.cargo/registry
      - ~/.cargo/git
      - target/
  script:
    - cargo install normalize --version "0.1.0" --locked
    - normalize ci
  artifacts:
    when: always
    reports:
      codequality: normalize.json
  after_script:
    - normalize ci --json > normalize.json || true
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

## Pinning the normalize Version

For reproducible CI, pin to a specific version and use `--locked` to respect
`Cargo.lock`:

```yaml
- run: cargo install normalize --version "0.1.0" --locked
```

Or install from a release binary once they are available:

```bash
curl -fsSL https://normalize.rs/install.sh | sh -s -- --version 0.1.0
```

Check for new versions with:

```bash
normalize update --check
```
