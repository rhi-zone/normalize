# CLI Usability Audit — Rules Commands

Audit of `normalize rules` command discoverability. Goal: users should never need to
read source code to configure rules.

## Gaps Found and Fixed

### 1. `normalize rules show <id>` — missing TOML config snippet (FIXED)

**Before:** Output included rule name, type, severity, enabled status, tags, langs,
allow list, message, and description — but no actionable config.

**After:** A "Configuration" section is appended at the end showing exactly what to
paste into `.normalize/config.toml`. The snippet uses the rule's current overrides
if any are set, or a copy-pasteable example otherwise. Example:

```
Configuration (.normalize/config.toml):
  [rules."rust/unwrap-in-impl"]
  severity = "warning"
  enabled = true
  allow = ["**/tests/**"]

  # Or use: normalize rules enable rust/unwrap-in-impl
  #         normalize rules disable rust/unwrap-in-impl
```

Implementation: `crates/normalize-rules/src/runner.rs`, `format_rule_show()`.

### 2. `normalize rules list` — no config discovery hints (FIXED)

**Before:** The list showed rules with type/severity/enabled/tags but gave no hint
about how to configure them.

**After:** A footer is added after the rule list:

```
Configure: [rules."<id>"] in .normalize/config.toml
  severity, enabled, allow — or: normalize rules enable/disable <id>
  Global patterns: [rules] global-allow = ["**/fixtures/**"]
  Custom tag groups: [rule-tags] my-group = ["tag1", "tag2"]
```

This surfaces the config section name, the available fields, and the two advanced
features (global-allow, rule-tags) that users would otherwise only discover by
reading `docs/rules.md`.

Implementation: `crates/normalize-rules/src/runner.rs`, `format_rule_list()`.

### 3. `normalize init --setup` — no config snippets (NOT YET FIXED)

The setup wizard prompts [e]nable/[d]isable/[s]kip for each rule but does not
show what TOML it will generate or emit a config block for copy-paste. Users
who want manual control (e.g. set severity to "warning" instead of "error") have
no guidance.

**Planned:** After the wizard runs, print a summary of all changes made as TOML
snippets so users can verify or manually replicate. Track in TODO.md.

## Remaining Gaps

- `normalize rules list` does not show the per-rule config key inline (only in the
  footer hint). Adding `[rules."<id>"]` as a second indented line per rule would
  make it even more copy-friendly, at the cost of visual density.
- `normalize rules show` allow list is shown verbatim (full current override list),
  which can be very long. A "current config" vs "example config" split might help.
- `normalize rules --help` does not mention `.normalize/config.toml` at all.
  A one-line "See also: .normalize/config.toml [rules] section" would help new users.

## Principle

**Every command should be self-documenting.** When a user runs `normalize rules show
<id>`, they should get everything needed to configure that rule — no external docs
required. When a user runs `normalize rules list`, they should get a clear path to
"how do I change one of these?".

The config section name changed from `[analyze.rules]` to `[rules]` in the
2026-03-09 refactor. Any documentation or snippets showing the old name are now
stale — the canonical name is `[rules]` and `[rules."<id>"]`.
