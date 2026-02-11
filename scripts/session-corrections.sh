#!/usr/bin/env bash
# Extract correction patterns from Claude Code session logs.
# Uses heuristics to find where Claude acknowledged mistakes.
#
# Usage: ./scripts/session-corrections.sh [session-pattern]
#        ./scripts/session-corrections.sh '*'        # all sessions
#        ./scripts/session-corrections.sh '3585*'    # specific session
#
# Primary use case: find patterns that should become CLAUDE.md rules

set -e

PATTERN="${1:-*}"

# Build once, run fast
cargo build --quiet --package normalize-cli --release 2>/dev/null
NORMALIZE="./target/release/normalize"

# Single pass: extract all Claude text responses with category tags
TMPFILE=$(mktemp)
$NORMALIZE sessions "$PATTERN" --jq '
  select(.type == "assistant") |
  .message.content[]? |
  select(.type == "text") |
  .text
' 2>/dev/null > "$TMPFILE"

echo "=== Immediate Acknowledgments ==="
grep -iE "^\"You.re right|^\"Good point|^\"Fair point|^\"Great point" "$TMPFILE" | head -15

echo
echo "=== Apologies / Mistakes ==="
grep -iE "I apologize|my mistake|I was wrong|I misunderstood|I should have|Sorry" "$TMPFILE" | head -15

echo
echo "=== Self-Corrections ==="
grep -iE "I.m (going in circles|making a mess|overcomplicating)|Actually,|Wait," "$TMPFILE" | head -15

echo
echo "=== Pattern Counts ==="
echo -n "You're right:     "; grep -ciE "^\"You.re right" "$TMPFILE" || echo 0
echo -n "Good/Fair point:  "; grep -ciE "^\"(Good|Fair|Great) point" "$TMPFILE" || echo 0
echo -n "Apologies:        "; grep -ciE "I apologize|my mistake|Sorry" "$TMPFILE" || echo 0
echo -n "Self-corrections: "; grep -ciE "I.m (going in circles|making a mess|overcomplicating)" "$TMPFILE" || echo 0

rm -f "$TMPFILE"

echo
echo "=== User Correction Messages (sample) ==="
echo "(What users say when correcting - for CLAUDE.md rules)"
echo
$NORMALIZE sessions "$PATTERN" --jq '
  select(.type == "user") |
  .message |
  select(.content | type == "string") |
  .content |
  select(test("(?i)(WTF|what the|wrong|wait[^i]|^no[,. ]|that.s not|shouldn.t|do you see|can you think|really now|why would|why are you|stop)"))
' 2>/dev/null | head -15
