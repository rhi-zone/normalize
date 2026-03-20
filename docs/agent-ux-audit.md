# Agent UX Audit: Compact Output Baseline

Cross-model audit of `--compact` output quality for agent consumption. Three models (Haiku, Sonnet, Opus) independently evaluated 12 commands. **Cross-model agreement (2+ models flagging the same issue) = genuine problem.** Single-model flags are lower priority and may reflect model-specific parsing preferences.

Date: 2026-03-20

## Methodology

Each model was given the same compact output samples and asked to rate parseability, token efficiency, and structural clarity. Ratings: GOOD (reliable machine parsing), MIXED (parseable with caveats), POOR (fragile or ambiguous).

## Prioritized Issues

Issues ordered by cross-model agreement count, then severity.

| # | Issue | Commands | Models | Severity |
|---|-------|----------|--------|----------|
| 1 | Truncated column headers (`WebFet`/`WebSea`) with no explanation | `sessions patterns` | Haiku, Sonnet, Opus | **High** — agents can't map truncated headers to API field names |
| 2 | No explicit message delimiters; multi-line content bleeds across boundaries | `sessions messages` | Haiku, Sonnet, Opus | **High** — boundary detection is fragile; multi-line tool output breaks naive parsing |
| 3 | Compound encoded fields (`80u 76tc`) — two values in one cell with opaque suffixes | `sessions list` | Haiku, Sonnet, Opus | **Medium** — requires regex `\d+u \d+tc` to decompose; field names lost |
| 4 | Format inconsistency between compact and JSON field names (`title` vs `first_message`) | `sessions list` | Opus (explicit), Sonnet (implicit via MIXED) | **Medium** — agents switching formats get different schemas |
| 5 | Silent empty output on missing/invalid argument (indistinguishable from zero results) | `analyze complexity` | Sonnet, Haiku (format-switching variant) | **High** — agents can't distinguish "no results" from "bad input" |
| 6 | Mixed formatting mid-output (prose then markdown, or mixed markdown styles) | `analyze complexity`, `sessions stats` | Haiku, Sonnet | **Medium** — parser must handle multiple formats in one response |
| 7 | `.` for zero values instead of `0` or `0.0%` | `sessions patterns` | Opus, Haiku (implicit) | **Low** — unusual but learnable convention |
| 8 | `row(s)` footer noise in query output | `structure query` | Opus | **Low** — minor; easily stripped |
| 9 | Bold markdown (`**label**`) adds tokens with no machine value | `sessions stats` | Opus, Sonnet | **Low** — 4 extra tokens per label, purely visual |
| 10 | No row count / completeness signal | `structure files` | Haiku, Sonnet | **Medium** — agent can't tell if output is truncated |
| 11 | `rank complexity` has no header row, ambiguous separator, variable-width score | `rank complexity` | Haiku | **Low** — Sonnet rated GOOD; likely model-specific |
| 12 | `rules list` — `off` as conditional token not fixed column | `rules list` | Sonnet | **Low** — Haiku rated GOOD; likely model-specific |

## Per-Command Consensus

| Command | Haiku | Sonnet | Opus | Consensus | Key Cross-Model Issues |
|---------|-------|--------|------|-----------|----------------------|
| `sessions list` | GOOD | MIXED | — | **MIXED** | Compound `80u 76tc` field; field name mismatch across formats |
| `sessions stats` | GOOD | POOR | — | **MIXED** | Bold markdown waste; mixed formatting styles |
| `sessions messages` | MIXED | MIXED | — | **MIXED** | No message delimiters; multi-line bleed (all three models) |
| `sessions patterns` | MIXED | MIXED | — | **MIXED** | Truncated headers (all three); `.` for zero |
| `structure query` | GOOD | GOOD | — | **GOOD** | Minor: `row(s)` footer, opaque `n` column |
| `structure stats` | GOOD | MIXED | — | **MIXED** | Unlabeled ratio/columns (Sonnet) |
| `structure files` | MIXED | POOR | — | **MIXED** | No row count; no metadata; opaque ordering |
| `analyze complexity` | MIXED | POOR | — | **POOR** | Silent empty output; format switching |
| `rank complexity` | POOR | GOOD | — | **SPLIT** | Disagreement — likely model-specific preferences |
| `view` | GOOD | POOR | — | **SPLIT** | Sonnet wants type info; Haiku finds it sufficient |
| `grep` | GOOD | GOOD | — | **GOOD** | Symbol context parenthetical praised by Sonnet |
| `rules list` | GOOD | MIXED | — | **MIXED** | Column parsing disagreement |

## Quick Wins

High cross-model agreement, likely easy to fix:

1. **Full column headers in `sessions patterns`** — don't truncate; or provide a header legend. All three models flagged this.
2. **Message delimiters in `sessions messages`** — add an explicit separator (e.g., `---` or `\0` in compact mode) between messages. All three models flagged fragile boundaries.
3. **Decompose compound fields in `sessions list`** — separate `usage` and `tool_calls` into distinct columns instead of `80u 76tc`.
4. **Non-silent errors in `analyze complexity`** — print a diagnostic line when input is missing/invalid instead of producing empty output.
5. **Consistent field names across `--compact` and `--json`** — `title` vs `first_message` should be the same concept with the same name.

## Deferred / Uncertain

Single-model flags or disagreements — investigate if patterns emerge:

- **`rank complexity` formatting** — Haiku rated POOR, Sonnet rated GOOD. Likely reflects different parsing strategies. Monitor.
- **`view` output richness** — Sonnet wants type info and metadata; Haiku finds plain path list sufficient. Revisit when `view` gets richer output anyway.
- **`rules list` column parsing** — Sonnet flagged `off` as non-fixed-width; Haiku found the table clear. May depend on parsing approach.
- **`.` for zero in pattern matrix** — Opus flagged as unusual. Technically unambiguous but unconventional. Low priority.
- **Bold markdown in stats** — wastes tokens but doesn't break parsing. Fix opportunistically.

## Meta-Observation

Opus's deepest insight: `--compact` is a human format marketed as an agent format. `--json` is always more reliable for programmatic consumers, but compact exists for token efficiency. The real fix is making compact genuinely machine-parseable (fixed-width, explicit delimiters, no markdown) rather than a pretty-printed human format with fewer words. This is a design-level tension, not a per-command bug.
